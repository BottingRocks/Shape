"""Portable operation classification (shape.ops.classify)."""

from typing import Any, Dict, List, Optional, Sequence

from ._native import JsonDoc, ShapeError, operation, text_document


def source(source: str) -> Dict[str, Any]:
    return operation("ops.classify.source", {"source": source})


def report(
    source: Optional[str] = None, *, report: Optional[JsonDoc] = None
) -> Dict[str, Any]:
    """Classify a serialized PipelineReport or reconstruct one from raw source."""
    if (source is None) == (report is None):
        raise ShapeError(
            "invalid_request", "classify.report needs exactly one of source or report"
        )
    return operation(
        "ops.classify.report",
        {"source": source, "report": text_document(report, "report")},
    )


def template(
    source: str,
    handler_index: Optional[int] = None,
    *,
    template: Optional[JsonDoc] = None,
) -> Dict[str, Any]:
    """Classify raw-source selected handler, or supplied template + handler text."""
    if (handler_index is None) == (template is None):
        raise ShapeError(
            "invalid_request",
            "classify.template needs exactly one of handler_index or template",
        )
    return operation(
        "ops.classify.template",
        {
            "source": source,
            "handler_index": handler_index,
            "template": text_document(template, "template"),
        },
    )


def seeds(seeds: Sequence[Dict[str, Any]]) -> List[Dict[str, Any]]:
    return operation("ops.classify.seeds", {"seeds": list(seeds)})
