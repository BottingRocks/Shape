//! The post-RENAME boundary between readable JavaScript and a raw Wasm ABI.
//!
//! JavaScript names are free to become readable.  Names in a WebAssembly
//! import/export section are not: they are part of the module's ABI.  This
//! module records the latter in one small ledger and, when the relation can be
//! proven from a parsed module, emits a JavaScript adapter using computed keys.
//! Unknown or dynamic modules are deliberately left alone.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::{AstNodes, ReferenceFlags, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use wasmparser::{ExternalKind, Imports, Parser, Payload, TypeRef, Validator};

use crate::transforms::deob::pipeline::decode_base64;
use crate::{
    analysis,
    runtime_decoder::{prove_runtime_decoder, STANDARD_ALPHABET, URL_SAFE_ALPHABET},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmAbiEntry {
    /// The raw import namespace. Empty for exports.
    pub namespace: String,
    pub raw_name: String,
    pub semantic_name: String,
    /// Structural ABI identity, retained independently of the raw spelling.
    pub kind: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WasmAbiLedger {
    pub namespaces: BTreeMap<String, String>,
    /// Every imported member, keyed by its exact namespace/member edge.
    pub imports: Vec<WasmAbiEntry>,
    pub host: Vec<WasmAbiEntry>,
    pub exports: Vec<WasmAbiEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WasmBindingReport {
    pub changed: bool,
    pub declined: bool,
    pub ledger: Option<WasmAbiLedger>,
    pub imports: Vec<(String, String)>,
    pub exports: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmBindingError {
    Parse(String),
}

impl std::fmt::Display for WasmBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "WASM binding parse failed: {error}"),
        }
    }
}

impl std::error::Error for WasmBindingError {}

#[derive(Debug, Clone)]
struct ModuleAbi {
    imports: Vec<AbiImport>,
    exports: Vec<AbiExport>,
}

#[derive(Debug, Clone)]
struct AbiImport {
    namespace: String,
    name: String,
    kind: ExternalKind,
    index: u32,
}

#[derive(Debug, Clone)]
struct AbiExport {
    name: String,
    kind: ExternalKind,
    index: u32,
}

#[derive(Debug, Clone)]
struct PropertyInfo {
    key: String,
    value_span: Span,
    value_is_callable: bool,
}

#[derive(Debug, Clone)]
struct ObjectInfo {
    properties: Vec<PropertyInfo>,
    has_non_data_property: bool,
}

#[derive(Debug, Clone)]
struct NamespaceInfo {
    raw_name: String,
    object: ObjectInfo,
}

#[derive(Debug, Clone)]
struct InstanceInfo {
    imports_span: Span,
    imports_prefix: Option<String>,
    imports: ObjectInfo,
    namespace_objects: Vec<NamespaceInfo>,
    module: ModuleAbi,
    insertion: u32,
}

fn report_imports(module: &ModuleAbi) -> Vec<(String, String)> {
    module
        .imports
        .iter()
        .map(|import| (import.namespace.clone(), import.name.clone()))
        .collect()
}

fn report_exports(module: &ModuleAbi) -> Vec<String> {
    module
        .exports
        .iter()
        .map(|export| export.name.clone())
        .collect()
}

/// Apply the single post-RENAME ABI hook.
pub fn post_rename(source: &str) -> Result<(String, WasmBindingReport), WasmBindingError> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(WasmBindingError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let decoders = proven_decoder_symbols(source, &nodes, &scoping);
    let modules = recover_modules(&nodes, &scoping, &decoders);
    let instances = recover_instances(source, &nodes, &scoping, &modules);
    let has_instance = nodes.iter().any(|node| {
        if let AstKind::NewExpression(expression) = node.kind() {
            is_webassembly_member(&expression.callee, "Instance", &scoping)
        } else {
            false
        }
    });
    if instances.is_empty() && has_instance {
        return Ok((
            source.to_owned(),
            WasmBindingReport {
                declined: true,
                diagnostics: vec!["opaque or dynamic WebAssembly.Instance relation declined".into()],
                ..WasmBindingReport::default()
            },
        ));
    }
    let usable: Vec<&InstanceInfo> = instances
        .iter()
        .filter(|instance| {
            has_abi(instance)
                && import_relation_is_complete(instance)
                && !host_namespace_names(instance).is_empty()
        })
        .collect();
    if usable.len() > 1 {
        return Ok((
            source.to_owned(),
            WasmBindingReport {
                declined: true,
                diagnostics: vec![
                    "multiple structurally valid WebAssembly.Instance ABI relations; identity is ambiguous"
                        .into(),
                ],
                ..WasmBindingReport::default()
            },
        ));
    }
    let Some(instance) = usable.into_iter().next() else {
        if has_instance {
            return Ok((
                source.to_owned(),
                WasmBindingReport {
                    declined: true,
                    diagnostics: vec![
                        "validated WebAssembly module has no usable import/export ABI".into(),
                    ],
                    ..WasmBindingReport::default()
                },
            ));
        }
        return Ok((source.to_owned(), WasmBindingReport::default()));
    };

    let ledger = ledger_for(instance);
    if instance.imports.has_non_data_property
        || instance
            .namespace_objects
            .iter()
            .any(|namespace| namespace.object.has_non_data_property)
    {
        return Ok((
            source.to_owned(),
            WasmBindingReport {
                declined: true,
                ledger: Some(ledger),
                imports: report_imports(&instance.module),
                exports: report_exports(&instance.module),
                diagnostics: vec![
                    "host import object uses accessor or method syntax; its access semantics cannot be preserved"
                        .into(),
                ],
                ..WasmBindingReport::default()
            },
        ));
    }
    let host_namespaces = host_namespace_names(instance);
    if host_namespaces.is_empty() {
        return Ok((
            source.to_owned(),
            WasmBindingReport {
                declined: true,
                ledger: Some(ledger),
                imports: report_imports(&instance.module),
                exports: report_exports(&instance.module),
                diagnostics: vec![
                    "validated module has no structurally proven host callback namespace".into(),
                ],
                ..WasmBindingReport::default()
            },
        ));
    }
    let host_bindings = render_host_bindings(
        source,
        instance,
        &host_namespaces,
        &ledger,
        instance.imports_prefix.as_deref(),
    );
    let imports = render_import_adapter(
        source,
        &instance.imports,
        instance,
        &host_namespaces,
        &ledger,
        instance.imports_prefix.as_deref(),
    );
    let ledger_source = render_ledger(&ledger);
    let insertion = if instance.imports_prefix.as_deref() == Some("vmHostImports") {
        format!("var vmWasmAbi = {ledger_source};\nvar vmHostBindings = {{ {host_bindings} }};\n")
    } else {
        format!(
            "var vmWasmAbi = {ledger_source};\nvar vmHostBindings = {{ {host_bindings} }};\nvar vmHostImports = {imports};\n"
        )
    };
    let argument_start = instance.imports_span.start;
    let argument_end = instance.imports_span.end;
    let edits = vec![
        analysis::Edit {
            start: instance.insertion,
            end: instance.insertion,
            replacement: insertion,
        },
        analysis::Edit {
            start: argument_start,
            end: argument_end,
            replacement: "vmHostImports".into(),
        },
    ];
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Ok((
            source.to_owned(),
            WasmBindingReport {
                declined: true,
                diagnostics: vec![format!(
                    "WASM ABI adapter declined: {}",
                    analysis::describe_rejected_edits(&applied.rejected)
                )],
                ..WasmBindingReport::default()
            },
        ));
    }
    let output = applied.output;
    Ok((
        output,
        WasmBindingReport {
            changed: true,
            declined: false,
            ledger: Some(ledger),
            imports: report_imports(&instance.module),
            exports: report_exports(&instance.module),
            diagnostics: vec![
                "validated raw WASM import/export ABI; emitted computed-key adapter".into(),
            ],
        },
    ))
}

fn recover_modules(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    decoders: &BTreeSet<SymbolId>,
) -> BTreeMap<SymbolId, ModuleAbi> {
    let mut bytes = BTreeMap::new();
    let mut declarations = BTreeMap::new();
    for node in nodes.iter() {
        if let AstKind::VariableDeclarator(declaration) = node.kind() {
            if let Some(id) = simple_binding(&declaration.id) {
                if let Some(symbol) = id.symbol_id.get() {
                    declarations.insert(symbol, declaration.init.as_ref());
                }
            }
        };
    }
    for (&symbol, initializer) in &declarations {
        let Some(Expression::NewExpression(expression)) = initializer else {
            continue;
        };
        if !is_webassembly_member(&expression.callee, "Module", scoping) {
            continue;
        }
        let Some(argument) = expression
            .arguments
            .first()
            .and_then(Argument::as_expression)
        else {
            continue;
        };
        let Some(raw) = static_bytes(argument, &declarations, scoping, decoders) else {
            continue;
        };
        let Some(abi) = parse_module(&raw) else {
            continue;
        };
        bytes.insert(symbol, abi);
    }
    bytes
}

fn recover_instances(
    source: &str,
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    modules: &BTreeMap<SymbolId, ModuleAbi>,
) -> Vec<InstanceInfo> {
    let mut declarations = BTreeMap::new();
    for node in nodes.iter() {
        if let AstKind::VariableDeclarator(declaration) = node.kind() {
            if let Some(id) = simple_binding(&declaration.id) {
                if let Some(symbol) = id.symbol_id.get() {
                    declarations.insert(symbol, declaration.init.as_ref());
                }
            }
        }
    }
    let mut out = Vec::new();
    for node in nodes.iter() {
        let AstKind::NewExpression(expression) = node.kind() else {
            continue;
        };
        if !is_webassembly_member(&expression.callee, "Instance", scoping) {
            continue;
        }
        let Some(Argument::Identifier(module)) = expression.arguments.first() else {
            continue;
        };
        let Some(module_symbol) = analysis::identifier_symbol(module, scoping) else {
            continue;
        };
        let Some(module_abi) = modules.get(&module_symbol) else {
            continue;
        };
        let Some(imports_expression) = expression
            .arguments
            .get(1)
            .and_then(Argument::as_expression)
        else {
            continue;
        };
        let (imports_expression, imports_prefix) = match imports_expression {
            Expression::ObjectExpression(object) => (&**object, None),
            Expression::Identifier(identifier) => {
                let Some(symbol) = analysis::identifier_symbol(identifier, scoping) else {
                    continue;
                };
                let Some(Some(Expression::ObjectExpression(object))) = declarations.get(&symbol)
                else {
                    continue;
                };
                (&**object, Some(identifier.name.to_string()))
            }
            _ => continue,
        };
        let Some(imports) = object_info_from_ast(imports_expression, &declarations, scoping) else {
            continue;
        };
        let namespace_objects = imports_expression
            .properties
            .iter()
            .filter_map(|property| {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                let raw_name = key_name(&property.key);
                let object = match &property.value {
                    Expression::ObjectExpression(object) => {
                        object_info_from_ast(object, &declarations, scoping)
                    }
                    Expression::Identifier(identifier) => {
                        let symbol = analysis::identifier_symbol(identifier, scoping)?;
                        match declarations.get(&symbol).and_then(|init| init.as_ref()) {
                            Some(Expression::ObjectExpression(object)) => {
                                object_info_from_ast(object, &declarations, scoping)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }?;
                Some(NamespaceInfo { raw_name, object })
            })
            .collect();
        out.push(InstanceInfo {
            imports_span: expression
                .arguments
                .get(1)
                .and_then(Argument::as_expression)
                .expect("instance import expression was just matched")
                .span(),
            imports_prefix,
            namespace_objects,
            imports,
            module: module_abi.clone(),
            insertion: insertion_point(nodes, expression.span.start)
                .unwrap_or(expression.span.start),
        });
    }
    let _ = source;
    out
}

fn has_abi(instance: &InstanceInfo) -> bool {
    !instance.module.imports.is_empty() && !instance.module.exports.is_empty()
}

fn import_relation_is_complete(instance: &InstanceInfo) -> bool {
    // One JavaScript property supplies every import with the same
    // namespace/member edge. A function and a table (or any other different
    // kinds) cannot safely share that property, so decline instead of
    // guessing which ABI role the renamed property represents.
    let mut edge_kinds = BTreeMap::<(&str, &str), ExternalKind>::new();
    for import in &instance.module.imports {
        if edge_kinds
            .insert(
                (import.namespace.as_str(), import.name.as_str()),
                import.kind,
            )
            .is_some_and(|kind| kind != import.kind)
        {
            return false;
        }
    }
    let mut namespaces = BTreeMap::<&str, BTreeSet<&str>>::new();
    for import in &instance.module.imports {
        namespaces
            .entry(import.namespace.as_str())
            .or_default()
            .insert(import.name.as_str());
    }
    for (namespace, names) in namespaces {
        if instance
            .namespace_objects
            .iter()
            .filter(|candidate| candidate.raw_name == namespace)
            .count()
            != 1
        {
            return false;
        }
        if !instance
            .imports
            .properties
            .iter()
            .any(|property| property.key == namespace)
        {
            return false;
        }
        let Some(object) = instance
            .namespace_objects
            .iter()
            .find(|candidate| candidate.raw_name == namespace)
            .map(|candidate| &candidate.object)
        else {
            return false;
        };
        let provided: BTreeSet<&str> = object
            .properties
            .iter()
            .map(|property| property.key.as_str())
            .collect();
        if !names.is_subset(&provided) {
            return false;
        }
    }
    true
}

/// A host namespace is proven from the ABI kind and the resolved JavaScript
/// values, never from its minified spelling.  Function imports must all be
/// present as callable values.  Table/memory/global namespaces are therefore
/// not accidentally selected as callback objects.
fn host_namespace_names(instance: &InstanceInfo) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut namespaces = BTreeSet::new();
    for import in &instance.module.imports {
        namespaces.insert(import.namespace.as_str());
    }
    for namespace in namespaces {
        let function_imports: Vec<&AbiImport> = instance
            .module
            .imports
            .iter()
            .filter(|import| {
                import.namespace == namespace
                    && matches!(import.kind, ExternalKind::Func | ExternalKind::FuncExact)
            })
            .collect();
        if function_imports.is_empty() {
            continue;
        }
        let Some(object) = instance
            .namespace_objects
            .iter()
            .find(|candidate| candidate.raw_name == namespace)
            .map(|candidate| &candidate.object)
        else {
            continue;
        };
        if function_imports.iter().all(|import| {
            object
                .properties
                .iter()
                .find(|property| property.key == import.name)
                .is_some_and(|property| property.value_is_callable)
        }) {
            candidates.push(namespace.to_owned());
        }
    }
    candidates
}

fn parse_module(bytes: &[u8]) -> Option<ModuleAbi> {
    Validator::new().validate_all(bytes).ok()?;
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut import_indices = BTreeMap::<&'static str, u32>::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.ok()? {
            Payload::ImportSection(reader) => {
                for group in reader {
                    match group.ok()? {
                        Imports::Single(_, import) => {
                            imports.push(abi_import(
                                import.module,
                                import.name,
                                import.ty,
                                &mut import_indices,
                            ));
                        }
                        Imports::Compact1 { module, items } => {
                            for item in items {
                                let item = item.ok()?;
                                imports.push(abi_import(
                                    module,
                                    item.name,
                                    item.ty,
                                    &mut import_indices,
                                ));
                            }
                        }
                        Imports::Compact2 { module, ty, names } => {
                            for name in names {
                                imports.push(abi_import(
                                    module,
                                    name.ok()?,
                                    ty,
                                    &mut import_indices,
                                ));
                            }
                        }
                    }
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.ok()?;
                    exports.push(AbiExport {
                        name: export.name.to_owned(),
                        kind: export.kind,
                        index: export.index,
                    });
                }
            }
            _ => {}
        }
    }
    Some(ModuleAbi { imports, exports })
}

fn abi_import(
    namespace: &str,
    name: &str,
    ty: TypeRef,
    indices: &mut BTreeMap<&'static str, u32>,
) -> AbiImport {
    let kind = match ty {
        TypeRef::Func(_) | TypeRef::FuncExact(_) => ExternalKind::Func,
        TypeRef::Table(_) => ExternalKind::Table,
        TypeRef::Memory(_) => ExternalKind::Memory,
        TypeRef::Global(_) => ExternalKind::Global,
        TypeRef::Tag(_) => ExternalKind::Tag,
    };
    let label = kind_label(kind);
    let index = *indices.entry(label).or_default();
    *indices.entry(label).or_default() += 1;
    AbiImport {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        kind,
        index,
    }
}

fn kind_label(kind: ExternalKind) -> &'static str {
    match kind {
        ExternalKind::Func | ExternalKind::FuncExact => "func",
        ExternalKind::Table => "table",
        ExternalKind::Memory => "memory",
        ExternalKind::Global => "global",
        ExternalKind::Tag => "tag",
    }
}

fn ledger_for(instance: &InstanceInfo) -> WasmAbiLedger {
    let mut ledger = WasmAbiLedger::default();
    let host_namespaces = host_namespace_names(instance);
    let mut raw_namespaces = Vec::new();
    for import in &instance.module.imports {
        if !raw_namespaces.contains(&import.namespace) {
            raw_namespaces.push(import.namespace.clone());
        }
    }
    let mut namespace_semantics = BTreeMap::new();
    let mut non_host_index = 0;
    for raw in raw_namespaces {
        let semantic = if host_namespaces.iter().any(|name| name == &raw) {
            let ordinal = host_namespaces
                .iter()
                .position(|name| name == &raw)
                .unwrap_or(0);
            if ordinal == 0 {
                "host".to_owned()
            } else {
                format!("hostNamespace{ordinal}")
            }
        } else {
            let semantic = format!("namespace{non_host_index}");
            non_host_index += 1;
            semantic
        };
        namespace_semantics.insert(raw, semantic);
    }
    for (raw, semantic) in &namespace_semantics {
        ledger.namespaces.insert(semantic.clone(), raw.clone());
    }
    for import in &instance.module.imports {
        let is_host = host_namespaces.iter().any(|name| name == &import.namespace);
        let semantic_name = if is_host && matches!(import.kind, ExternalKind::Func) {
            format!("hostCallback{}", import.index)
        } else {
            format!("wasmImport{}{}", kind_label(import.kind), import.index)
        };
        let entry = WasmAbiEntry {
            namespace: import.namespace.clone(),
            raw_name: import.name.clone(),
            semantic_name,
            kind: kind_label(import.kind).to_owned(),
            index: import.index,
        };
        ledger.imports.push(entry.clone());
        if is_host {
            ledger.host.push(entry);
        }
    }
    for export in &instance.module.exports {
        // Export names are edges, not entities: one function/table can have
        // several export aliases. Keep the entity identity in kind/index and
        // add the export-section ordinal to the ledger key so aliases do not
        // overwrite one another when the ledger is rendered.
        let ordinal = ledger.exports.len();
        ledger.exports.push(WasmAbiEntry {
            namespace: String::new(),
            raw_name: export.name.clone(),
            semantic_name: format!(
                "wasmExport{ordinal}{}{index}",
                kind_label(export.kind),
                index = export.index
            ),
            kind: kind_label(export.kind).to_owned(),
            index: export.index,
        });
    }
    ledger
}

fn render_ledger(ledger: &WasmAbiLedger) -> String {
    let namespaces = ledger
        .namespaces
        .iter()
        .map(|(semantic, raw)| format!("{semantic}: \"{}\"", js_string(raw)))
        .collect::<Vec<_>>()
        .join(", ");
    let host = ledger
        .host
        .iter()
        .map(|entry| {
            format!(
                "{}: \"{}\"",
                entry.semantic_name,
                js_string(&entry.raw_name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let imports = ledger
        .imports
        .iter()
        .map(|entry| {
            format!(
                "{}: \"{}\"",
                entry.semantic_name,
                js_string(&entry.raw_name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let exports = ledger
        .exports
        .iter()
        .map(|entry| {
            format!(
                "{}: \"{}\"",
                entry.semantic_name,
                js_string(&entry.raw_name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{ namespace: {{{namespaces}}}, imports: {{{imports}}}, host: {{{host}}}, exports: {{{exports}}} }}"
    )
}

fn property_source(source: &str, property: &PropertyInfo) -> String {
    source[property.value_span.start as usize..property.value_span.end as usize].to_owned()
}

fn render_host_bindings(
    source: &str,
    instance: &InstanceInfo,
    host_namespaces: &[String],
    ledger: &WasmAbiLedger,
    imports_prefix: Option<&str>,
) -> String {
    let mut rendered = Vec::new();
    for (host_index, namespace) in host_namespaces.iter().enumerate() {
        let Some(object) = instance
            .namespace_objects
            .iter()
            .find(|candidate| candidate.raw_name == *namespace)
            .map(|candidate| &candidate.object)
        else {
            continue;
        };
        for (property_index, property) in object.properties.iter().enumerate() {
            let entry = ledger
                .imports
                .iter()
                .find(|entry| entry.namespace == *namespace && entry.raw_name == property.key);
            if entry.is_none() && imports_prefix.is_some() {
                continue;
            }
            let value = entry
                .map(|_| {
                    imports_prefix
                        .map(|prefix| {
                            format!(
                                "{prefix}[{}][\"{}\"]",
                                namespace_expression(ledger, namespace),
                                js_string(&property.key)
                            )
                        })
                        .unwrap_or_else(|| property_source(source, property))
                })
                .unwrap_or_else(|| property_source(source, property));
            let key = entry
                .map(|entry| format!("\"{}\"", js_string(&entry.semantic_name)))
                .unwrap_or_else(|| format!("\"hostExtra{host_index}_{property_index}\""));
            rendered.push(format!(
                "[{key}]: {}",
                remove_generated_function_name(&value)
            ));
        }
    }
    rendered.join(", ")
}

fn remove_generated_function_name(value: &str) -> String {
    let Some(rest) = value.strip_prefix("function ") else {
        return value.to_owned();
    };
    let Some(open) = rest.find('(') else {
        return value.to_owned();
    };
    let name = &rest[..open];
    if name.starts_with("vmHelper_") {
        return format!("function{}", &rest[open..]);
    }
    value.to_owned()
}

fn render_import_adapter(
    source: &str,
    imports: &ObjectInfo,
    instance: &InstanceInfo,
    host_namespaces: &[String],
    ledger: &WasmAbiLedger,
    imports_prefix: Option<&str>,
) -> String {
    let mut values = Vec::new();
    for property in &imports.properties {
        let key = namespace_expression(ledger, &property.key);
        let value = if host_namespaces
            .iter()
            .any(|namespace| namespace == &property.key)
        {
            let Some(object) = instance
                .namespace_objects
                .iter()
                .find(|candidate| candidate.raw_name == property.key)
                .map(|candidate| &candidate.object)
            else {
                continue;
            };
            let host_index = host_namespaces
                .iter()
                .position(|namespace| namespace == &property.key)
                .unwrap_or(0);
            render_host_namespace(object, ledger, imports_prefix, &property.key, host_index)
        } else if let Some(object) = instance
            .namespace_objects
            .iter()
            .find(|candidate| candidate.raw_name == property.key)
            .map(|candidate| &candidate.object)
        {
            render_namespace_object(source, object, ledger, imports_prefix, &property.key)
        } else {
            property_source(source, property)
        };
        values.push(format!("[{key}]: {value}"));
    }
    format!("{{ {} }}", values.join(", "))
}

fn namespace_expression(ledger: &WasmAbiLedger, raw: &str) -> String {
    ledger
        .namespaces
        .iter()
        .find(|(_, candidate)| candidate.as_str() == raw)
        .map(|(semantic, _)| format!("vmWasmAbi.namespace.{semantic}"))
        .unwrap_or_else(|| format!("\"{}\"", js_string(raw)))
}

fn render_host_namespace(
    object: &ObjectInfo,
    ledger: &WasmAbiLedger,
    imports_prefix: Option<&str>,
    namespace: &str,
    host_index: usize,
) -> String {
    let properties = object
        .properties
        .iter()
        .enumerate()
        .map(|(property_index, property)| {
            let entry = ledger
                .imports
                .iter()
                .find(|entry| entry.namespace == namespace && entry.raw_name == property.key);
            let binding_key = entry
                .map(|entry| format!("\"{}\"", js_string(&entry.semantic_name)))
                .unwrap_or_else(|| format!("\"hostExtra{host_index}_{property_index}\""));
            let abi_key = entry
                .map(|entry| format!("vmWasmAbi.imports.{}", entry.semantic_name))
                .unwrap_or_else(|| format!("\"{}\"", js_string(&property.key)));
            let value = if entry.is_some() {
                format!("vmHostBindings[{binding_key}]")
            } else if let Some(prefix) = imports_prefix {
                format!(
                    "{prefix}[{}][\"{}\"]",
                    namespace_expression(ledger, namespace),
                    js_string(&property.key)
                )
            } else {
                format!("vmHostBindings[{binding_key}]")
            };
            format!("[{abi_key}]: {value}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {properties} }}")
}

fn render_namespace_object(
    source: &str,
    object: &ObjectInfo,
    ledger: &WasmAbiLedger,
    imports_prefix: Option<&str>,
    namespace: &str,
) -> String {
    let properties = object
        .properties
        .iter()
        .map(|property| {
            let entry = ledger
                .imports
                .iter()
                .find(|entry| entry.namespace == namespace && entry.raw_name == property.key);
            let key = entry
                .map(|entry| format!("vmWasmAbi.imports.{}", entry.semantic_name))
                .unwrap_or_else(|| format!("\"{}\"", js_string(&property.key)));
            let value = imports_prefix
                .map(|prefix| {
                    format!(
                        "{prefix}[{}][\"{}\"]",
                        namespace_expression(ledger, namespace),
                        js_string(&property.key)
                    )
                })
                .unwrap_or_else(|| property_source(source, property));
            format!("[{key}]: {value}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {properties} }}")
}

fn object_info_from_ast(
    object: &ObjectExpression<'_>,
    declarations: &BTreeMap<SymbolId, Option<&Expression<'_>>>,
    scoping: &Scoping,
) -> Option<ObjectInfo> {
    let mut properties = Vec::new();
    let mut has_non_data_property = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        let key = key_name(&property.key);
        if key.is_empty() || property.shorthand {
            return None;
        }
        // Accessors and method shorthand carry a `(params) { body }` value span
        // that is only legal as method syntax, never as a property value. The
        // adapter copies value spans, so such an object cannot be re-emitted
        // faithfully and must be declined instead.
        if property.kind != PropertyKind::Init || property.method {
            has_non_data_property = true;
        }
        properties.push(PropertyInfo {
            key,
            value_span: property.value.span(),
            value_is_callable: expression_is_callable(&property.value, declarations, scoping),
        });
    }
    Some(ObjectInfo {
        properties,
        has_non_data_property,
    })
}

fn expression_is_callable(
    expression: &Expression<'_>,
    declarations: &BTreeMap<SymbolId, Option<&Expression<'_>>>,
    scoping: &Scoping,
) -> bool {
    expression_is_callable_inner(expression, declarations, scoping, &mut BTreeSet::new())
}

fn expression_is_callable_inner(
    expression: &Expression<'_>,
    declarations: &BTreeMap<SymbolId, Option<&Expression<'_>>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> bool {
    match expression {
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::Identifier(identifier) => {
            let Some(symbol) = analysis::identifier_symbol(identifier, scoping) else {
                return false;
            };
            if !seen.insert(symbol) {
                return false;
            }
            declarations
                .get(&symbol)
                .and_then(|init| init.as_ref())
                .is_some_and(|init| expression_is_callable_inner(init, declarations, scoping, seen))
        }
        _ => false,
    }
}

fn insertion_point(nodes: &AstNodes<'_>, start: u32) -> Option<u32> {
    let node = nodes.iter().find(|node| node.span().start == start)?;
    for ancestor in nodes.ancestors(node.id()) {
        match ancestor.kind() {
            AstKind::ExpressionStatement(statement) => return Some(statement.span.start),
            AstKind::VariableDeclaration(declaration) => return Some(declaration.span.start),
            _ => {}
        }
        if let AstKind::Function(function) = ancestor.kind() {
            return function.body.as_ref().map(|body| body.span.start + 1);
        }
    }
    None
}

fn static_bytes(
    expression: &Expression<'_>,
    declarations: &BTreeMap<SymbolId, Option<&Expression<'_>>>,
    scoping: &Scoping,
    decoders: &BTreeSet<SymbolId>,
) -> Option<Vec<u8>> {
    match expression {
        Expression::Identifier(id) => {
            let symbol = analysis::identifier_symbol(id, scoping)?;
            static_bytes(
                declarations.get(&symbol)?.as_ref()?,
                declarations,
                scoping,
                decoders,
            )
        }
        Expression::CallExpression(call) => {
            let Expression::Identifier(callee) = &call.callee else {
                return None;
            };
            let symbol = analysis::identifier_symbol(callee, scoping)?;
            if !decoders.contains(&symbol) {
                return None;
            }
            let value = call.arguments.first()?.as_expression()?;
            let Expression::StringLiteral(value) = value else {
                return None;
            };
            decode_base64(value.value.as_str()).ok()
        }
        Expression::NewExpression(new) => {
            let identifier = match &new.callee {
                Expression::Identifier(id) => id,
                _ => return None,
            };
            if !matches!(identifier.name.as_str(), "Uint8Array" | "Array")
                || analysis::identifier_symbol(identifier, scoping).is_some()
            {
                return None;
            }
            let values = new.arguments.first()?.as_expression()?;
            static_number_array(values)
        }
        _ => None,
    }
}

/// Prove decoder bindings once for this post-RENAME pass.  The proof is tied
/// to the resolved binding and exact function span; source spelling is never
/// consulted.  Stable aliases are accepted only when they resolve to a proven
/// function binding, while unresolved globals (including bare `atob`) decline.
/// Either proven alphabet is accepted: a kernel may decode its WebAssembly
/// module with the URL-safe decoder, and the module literal is decoded with
/// the alphabet the proof names.
fn proven_decoder_symbols(
    source: &str,
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
) -> BTreeSet<SymbolId> {
    let mut declarations = BTreeMap::new();
    let mut candidate_spans = BTreeMap::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::Function(function) => {
                if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                    candidate_spans.insert(symbol, function.span);
                }
            }
            AstKind::VariableDeclarator(declaration) => {
                let Some(binding) = simple_binding(&declaration.id) else {
                    continue;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    continue;
                };
                declarations.insert(symbol, declaration.init.as_ref());
                if let Some(Expression::FunctionExpression(function)) = declaration.init.as_ref() {
                    candidate_spans.insert(symbol, function.span);
                    if let Some(function_symbol) =
                        function.id.as_ref().and_then(|id| id.symbol_id.get())
                    {
                        candidate_spans.insert(function_symbol, function.span);
                    }
                }
            }
            _ => {}
        }
    }

    let mut proof_cache = BTreeMap::<(u32, u32), bool>::new();
    let mut proven = BTreeSet::new();
    for (symbol, span) in candidate_spans {
        if scoping.symbol_declarations(symbol).count() != 1
            || scoping
                .get_resolved_references(symbol)
                .any(|reference| reference.flags().contains(ReferenceFlags::Write))
        {
            continue;
        }
        let Some(function_source) = source.get(span.start as usize..span.end as usize) else {
            continue;
        };
        // The module is decoded from the standard or the URL-safe alphabet,
        // whichever the decoder proof names.
        if !function_source.contains(STANDARD_ALPHABET)
            && !function_source.contains(URL_SAFE_ALPHABET)
        {
            continue;
        }
        let key = (span.start, span.end);
        let proof = proof_cache
            .entry(key)
            .or_insert_with(|| prove_runtime_decoder(source, span).is_some());
        if *proof {
            proven.insert(symbol);
        }
    }

    let aliases: BTreeMap<SymbolId, SymbolId> = declarations
        .into_iter()
        .filter(|(symbol, _)| {
            scoping.symbol_declarations(*symbol).count() == 1
                && !scoping
                    .get_resolved_references(*symbol)
                    .any(|reference| reference.flags().contains(ReferenceFlags::Write))
        })
        .filter_map(|(symbol, init)| {
            let Expression::Identifier(identifier) = init? else {
                return None;
            };
            let target = analysis::identifier_symbol(identifier, scoping)?;
            Some((symbol, target))
        })
        .collect();
    let mut resolved = BTreeSet::new();
    for symbol in aliases.keys().copied() {
        if resolves_proven_decoder(symbol, &aliases, &proven, &mut BTreeSet::new()) {
            resolved.insert(symbol);
        }
    }
    proven.extend(resolved);
    proven
}

fn resolves_proven_decoder(
    symbol: SymbolId,
    aliases: &BTreeMap<SymbolId, SymbolId>,
    proven: &BTreeSet<SymbolId>,
    seen: &mut BTreeSet<SymbolId>,
) -> bool {
    if proven.contains(&symbol) {
        return true;
    }
    if !seen.insert(symbol) {
        return false;
    }
    aliases
        .get(&symbol)
        .is_some_and(|target| resolves_proven_decoder(*target, aliases, proven, seen))
}

fn static_number_array(expression: &Expression<'_>) -> Option<Vec<u8>> {
    let Expression::ArrayExpression(array) = expression else {
        return None;
    };
    array
        .elements
        .iter()
        .map(|element| match element {
            ArrayExpressionElement::NumericLiteral(value) => u8::try_from(value.value as u64).ok(),
            _ => None,
        })
        .collect()
}

fn is_webassembly_member(expression: &Expression<'_>, property: &str, scoping: &Scoping) -> bool {
    let Expression::StaticMemberExpression(member) = expression else {
        return false;
    };
    let Expression::Identifier(object) = &member.object else {
        return false;
    };
    object.name == "WebAssembly"
        && member.property.name == property
        && analysis::identifier_symbol(object, scoping).is_none()
}

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id),
        _ => None,
    }
}

fn key_name(key: &PropertyKey<'_>) -> String {
    key.name().map(|name| name.into_owned()).unwrap_or_default()
}

fn js_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
