// Closed owned installer grammar. External consumer roles carry no native identity.
function environment(callThrough, registry, hookFactory, constructConsumer, invokeConsumer, sourceObject, sources, wrappers, excluded) {
function installer(input, key, option) {
			let constructorOption = option;
			let before, original;
			if (option === void 0) {
				constructorOption = false;
			}
			let cursor = input;
			let descriptorRecord;
			for (;;) {
				descriptorRecord = void 0;
				if (cursor == null) {
					descriptorRecord = null;
					break;
				}
				let descriptor = Object.getOwnPropertyDescriptor(cursor, key);
				if (descriptor == null) {
					cursor = Object.getPrototypeOf(cursor);
					continue;
				}
				descriptorRecord = {
					containingObj: cursor,
					desc: descriptor
				};
				break;
			}
			if (descriptorRecord == null) {
				return null;
			}
			let owner = descriptorRecord.containingObj;
			descriptorRecord = descriptorRecord.desc;
			let configurable = descriptorRecord.configurable;
			let writable = descriptorRecord.writable;
			original = descriptorRecord.value;
			if (!callThrough(Object.hasOwnProperty, descriptorRecord, "value")) {
				return null;
			}
			let originals = Object.create(null);
			originals.value = original;
			if (configurable === false && writable === false || typeof original !== "function") {
				originals = { originals: originals };
				return originals;
			} else {
				let selected, cached;
				if (registry[key] == null) {
					selected = null;
					cached = null;
				} else {
					let scanCursor = 0;
					for (;;) {
						selected = void 0;
						if (scanCursor >= registry[key].length) {
							selected = null;
							break;
						}
						selected = registry[key][scanCursor];
						if (owner !== selected.object) {
							++scanCursor;
							continue;
						}
						break;
					}
					cached = selected;
				}
				if (selected != null) {
					if (cached.result != null) {
						return cached.result;
					}
					original = cached.original;
					originals.value = original;
				}
				before = hookFactory();
				let after = hookFactory();
				function wrapper() {
					"use strict";
					let actualArgs = arguments;
					let beforeResult = before.notify({
						args: actualArgs,
						thisObj: this
					});
					let effectiveArgs;
					if (beforeResult) {
						if (beforeResult.bypassResult != null) {
							if (beforeResult.bypassResult.throw) {
								throw beforeResult.bypassResult.value;
							} else {
								return beforeResult.bypassResult.value;
							}
						} else {
							effectiveArgs = beforeResult.args != null ? beforeResult.args : actualArgs;
						}
					} else {
						effectiveArgs = actualArgs;
					}
					actualArgs = arguments;
					let invocationResult;
					let payload = {
						args: actualArgs,
						thisObj: this,
						threw: true,
						result: null
					};
					let finalResult;
					try {
						invocationResult = constructorOption && this instanceof wrapper ? constructConsumer(original, effectiveArgs) : invokeConsumer(original, this, effectiveArgs);
						payload = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: invocationResult
						};
					} finally {
						let afterResult = after.notify(payload);
						if (afterResult && afterResult.bypassResult != null) {
							if (afterResult.bypassResult.throw) {
								throw afterResult.bypassResult.value;
							} else {
								return afterResult.bypassResult.value;
							}
						} else {
							finalResult = invocationResult;
						}
					}
					return finalResult;
				}
				descriptorRecord.value = wrapper;
				let installed = descriptorRecord.value;
				if (typeof original === "function") {
					try {
						let source = sourceObject.toString.call(original);
						sources.push(source);
						wrappers.push(installed);
						if (Function.prototype.toString !== sourceObject.toString) {
							Function.prototype.toString = sourceObject.toString;
						}
					} catch (e) {}
				}
				let names = Object.getOwnPropertyNames(original);
				let i = 0;
				for (; i < names.length; i++) {
					if (callThrough(Array.prototype.indexOf, excluded, names[i]) === -1) {
						let installedDescriptor = Object.getOwnPropertyDescriptor(installed, names[i]);
						if (installedDescriptor == null || installedDescriptor.configurable === true || installedDescriptor.writable === true) {
							let originalDescriptor = Object.getOwnPropertyDescriptor(original, names[i]);
							if (originalDescriptor != null) {
								if (installedDescriptor == null || installedDescriptor.configurable !== false || originalDescriptor.configurable !== true) {
									Object.defineProperty(installed, names[i], originalDescriptor);
								}
							}
						}
					}
				}
				try {
					if (!callThrough(Object.hasOwnProperty, original, "prototype")) {
						installed.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty(owner, key, descriptorRecord);
				originals = {
					onBeforeInvoke: before,
					onAfterInvoke: after,
					originals: originals
				};
				owner = {
					object: owner,
					result: originals,
					original: original
				};
				let bucket = registry[key];
				if (bucket == null) {
					bucket = [];
					registry[key] = bucket;
				}
				callThrough(Array.prototype.push, bucket, owner);
				return originals;
			}
		}

}
environment();
