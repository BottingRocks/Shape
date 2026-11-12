use crate::signals::schema::ProgramPropertiesLimits;
use super::*;

const PREFIX: &str = "function bootstrap() {\n";

fn prove(source: &str) -> Vec<FinderClaim> {
    let wrapped = format!("{PREFIX}{source}\n}} bootstrap();");
    let source = wrapped.as_str();
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let model = Model::new(
        source,
        &parsed.program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    claims(&model)
}
fn accepts(source: &str, role: &str) {
    let claims = prove(source);
    assert_eq!(claims.len(), 1, "{source}");
    let FinderClaim::ClassBinding {
        role: actual,
        class_span,
        ..
    } = &claims[0]
    else {
        panic!("function gain")
    };
    assert_eq!(*actual, role);
    assert!(source
        [class_span.start as usize - PREFIX.len()..class_span.end as usize - PREFIX.len()]
        .starts_with("class "));
}
fn refuses(source: &str) {
    assert!(prove(source).is_empty(), "{source}");
}

#[test]
fn complete_own_lifecycle_and_alpha_renamed_locals() {
    for (source, role) in [
        (SUBSCRIPTION, "subscription_class"),
        (OBSERVER, "subscription_observer_class"),
    ] {
        accepts(source, role);
        let renamed = source
            .replace("SubscriptionObserver", "$THREAD_8_2")
            .replace("Subscription", "$THREAD_2_3")
            .replace("subscription)", "$first)")
            .replace("= subscription;", "= $first;")
            .replace("observer", "renamedObserver")
            .replace("_renamedObserver", "_observer")
            .replace("cleanup", "renamedCleanup")
            .replace("_renamedCleanup", "_cleanup")
            .replace("caught", "exceptionValue");
        accepts(&renamed, role);
    }
}

#[test]
fn optional_closed_getter_and_method_order() {
    for (source, role, getter) in [
        (
            SUBSCRIPTION,
            "subscription_class",
            "get closed() { return this._observer === void 0; }",
        ),
        (
            OBSERVER,
            "subscription_observer_class",
            "get closed() { return this._subscription._observer === void 0; }",
        ),
    ] {
        accepts(&source.replace(getter, ""), role);
        let mut moved = source.replace(getter, "");
        let opening = moved.find('{').unwrap() + 1;
        moved.insert_str(opening, getter);
        accepts(&moved, role);
        refuses(&source.replace(getter, &getter.replace("===", "!==")));
        refuses(&source.replace("get closed()", "static get closed()"));
        refuses(&source.replace("get closed()", "closed()"));
    }
}

#[test]
fn refuses_false_state_sources_receivers_arguments_and_order() {
    for (source, old, new) in [
        (
            SUBSCRIPTION,
            "this._observer = observer",
            "this._observer = void 0",
        ),
        (
            SUBSCRIPTION,
            "this._cleanup = void 0; this._observer",
            "this._cleanup = observer; this._observer",
        ),
        (SUBSCRIPTION, "this.isComplete()", "other.isComplete()"),
        (
            SUBSCRIPTION,
            "this._observer === void 0",
            "this._cleanup === void 0",
        ),
        (
            SUBSCRIPTION,
            "this._cleanup = void 0;\n                try { cleanup(); }",
            "try { cleanup(); this._cleanup = void 0; }",
        ),
        (
            OBSERVER,
            "this._subscription = subscription",
            "this._subscription = this",
        ),
        (
            OBSERVER,
            "this._subscription._observer.next(input)",
            "this._subscription._observer.next(void 0)",
        ),
        (
            OBSERVER,
            "this._subscription._observer.next(input)",
            "other.next(input)",
        ),
        (
            OBSERVER,
            "let sub = this._subscription;",
            "let sub = this._subscription; sub = other;",
        ),
        (
            OBSERVER,
            "sub._observer = void 0;",
            "other._observer = void 0;",
        ),
        (
            OBSERVER,
            "sub._cleanup = void 0;",
            "sub._observer = void 0;",
        ),
        (
            OBSERVER,
            "observer.error(value)",
            "observer.error(input, value)",
        ),
        (OBSERVER, ".call(observer)", ".call(sub)"),
        (
            OBSERVER,
            "this._subscription.isComplete()",
            "this.isComplete()",
        ),
    ] {
        assert!(source.contains(old));
        refuses(&source.replace(old, new));
    }
}

#[test]
fn bundled_callback_requires_exact_receiver_and_indices() {
    let bundled = OBSERVER.replace(
        "observer.error(value);",
        "let pair = [observer.error, observer.error.call]; pair[1].call(pair[0], observer, value);",
    );
    accepts(&bundled, "subscription_observer_class");
    for (from, to) in [
        ("pair[1]", "pair[2]"),
        ("pair[0]", "pair[1]"),
        ("pair[0], observer, value", "pair[0], sub, value"),
        ("observer.error.call", "observer.complete.call"),
    ] {
        refuses(&bundled.replace(from, to));
    }
}

#[test]
fn refuses_donors_and_ambiguous_native_api() {
    for source in [SUBSCRIPTION, OBSERVER] {
        let mut duplicate = source.to_string();
        duplicate.insert_str(
            duplicate.rfind('}').unwrap(),
            "isComplete() { return true; }",
        );
        refuses(&duplicate);
        refuses(&source.replace("isComplete() {", "static isComplete() {"));
        refuses(&source.replace("isComplete() {", "get isComplete() {"));
        refuses(&source.replace("isComplete() {", "['isComplete']() {"));
        refuses(&source.replace("isComplete() {", "async isComplete() {"));
        let name = if source == SUBSCRIPTION {
            "Subscription"
        } else {
            "SubscriptionObserver"
        };
        refuses(&source.replace(
            &format!("class {name} {{"),
            &format!("class {name} extends Other {{"),
        ));
        let mut nested = source.to_string();
        nested.insert_str(
            nested.rfind('}').unwrap(),
            "donor() { function nested() { this._observer = void 0; } }",
        );
        refuses(&nested);
    }
    let donor = SUBSCRIPTION
        .replace("unsubscribe() {", "unsubscribe() { function donor() {")
        .replace("    isComplete()", "    } isComplete()");
    refuses(&donor);
    refuses("class Fake { constructor(x) { this._cleanup = void 0; this._observer = x; } unsubscribe() {} isComplete() {} }");
    let siblings = format!(
        "{} class Fake {{ next(x) {{}} error(x) {{}} complete() {{}} }}",
        SUBSCRIPTION
    );
    assert_eq!(prove(&siblings).len(), 1);
}

#[test]
fn refuses_overwrites_through_native_class_and_aliases() {
    for extra in [
        "Subscription.prototype.unsubscribe = 0;",
        "Subscription.prototype = {};",
        "delete Subscription.prototype.unsubscribe;",
        "Object.defineProperty(Subscription.prototype, 'unsubscribe', {});",
        "let alias = Subscription; alias.prototype.unsubscribe = function() {};",
        "let alias = Subscription; alias = Other;",
        "Subscription.prototype[unknown] = 0;",
    ] {
        refuses(&format!("{SUBSCRIPTION}\n{extra}"));
    }
    accepts(
        &format!("{SUBSCRIPTION}\nlet alias = Subscription; new alias({{}});"),
        "subscription_class",
    );
}

// Exact class fixtures from the pinned static inventory; these test the proof, not routing.

#[test]
fn subscription_unsubscribe_clears_observer_and_cleanup() {
    accepts(
        r##"class $THREAD_2_15637 {
			constructor($BR) {
				this._cleanup = void 0;
				this._observer = $BR;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $BR = this._cleanup;
					if ($BR !== void 0) {
						this._cleanup = void 0;
						try {
							$BR();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
		}"##,
        "subscription_class",
    );
}

#[test]
fn observer_error_call_holder_and_error_first_completion_flags() {
    accepts(
        r##"class $THREAD_9_157835 {
			constructor($BR) {
				this._subscription = $BR;
			}
			next($BR) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BR);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BR) {
				let $KEY_2 = this._subscription;
				let $BS = $KEY_2._observer;
				$KEY_2._observer = void 0;
				try {
					let $BT = $KEY_2._cleanup;
					let $BU;
					if ($BT === void 0) {
						$BU = $BR;
					} else {
						$KEY_2._cleanup = void 0;
						try {
							$BT();
							$BU = $BR;
						} catch (e) {
							$BU = $BR;
						}
					}
					if ($BS) {
						if ($BS.error) {
							let $BV = [$BS.error, $BS.error.call];
							$BV[1].call($BV[0], $BS, $BU);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_5 = this._subscription;
				let $BR = $KEY_5._observer;
				if ($KEY_5._observer === void 0) {
					return void 0;
				}
				$KEY_5._observer = void 0;
				try {
					if (typeof ($BR ? $BR.complete : void 0) === "function") {
						($BR ? $BR.complete : void 0).call($BR);
					}
				} catch (e) {}
				let $BS, $BT;
				try {
					let $BU = $KEY_5._cleanup;
					if ($BU !== void 0) {
						$KEY_5._cleanup = void 0;
						try {
							$BU();
						} catch (e_2) {}
					}
					$BS = void 0;
					$BT = false;
				} catch (e_3) {
					$BS = e_3;
					$BT = true;
				}
				if ($BT) {
					throw $BS;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_direct_error_call_and_error_first_completion_flags() {
    accepts(
        r##"class $THREAD_2_134249 {
			constructor($BT) {
				this._subscription = $BT;
			}
			next($BT) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BT);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BT) {
				let $KEY_3 = this._subscription;
				let $BU = $KEY_3._observer;
				$KEY_3._observer = void 0;
				try {
					let $BV = $KEY_3._cleanup;
					let $BW;
					if ($BV === void 0) {
						$BW = $BT;
					} else {
						$KEY_3._cleanup = void 0;
						try {
							$BV();
							$BW = $BT;
						} catch (e) {
							$BW = $BT;
						}
					}
					if ($BU) {
						if ($BU.error) {
							$BU.error($BW);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_3 = this._subscription;
				let $BT = $KEY_3._observer;
				if ($KEY_3._observer === void 0) {
					return void 0;
				}
				$KEY_3._observer = void 0;
				try {
					if (typeof ($BT ? $BT.complete : void 0) === "function") {
						($BT ? $BT.complete : void 0).call($BT);
					}
				} catch (e) {}
				let $BU, $BV;
				try {
					let $BW = $KEY_3._cleanup;
					if ($BW !== void 0) {
						$KEY_3._cleanup = void 0;
						try {
							$BW();
						} catch (e_2) {}
					}
					$BU = void 0;
					$BV = false;
				} catch (e_3) {
					$BU = e_3;
					$BV = true;
				}
				if ($BV) {
					throw $BU;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_direct_error_call_and_flag_first_completion_flags() {
    accepts(
        r##"class $THREAD_0_94586 {
			constructor($BT) {
				this._subscription = $BT;
			}
			next($BT) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BT);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BT) {
				let $KEY_4 = this._subscription;
				let $BU = $KEY_4._observer;
				$KEY_4._observer = void 0;
				try {
					let $BV = $KEY_4._cleanup;
					let $BW;
					if ($BV === void 0) {
						$BW = $BT;
					} else {
						$KEY_4._cleanup = void 0;
						try {
							$BV();
							$BW = $BT;
						} catch (e) {
							$BW = $BT;
						}
					}
					if ($BU) {
						if ($BU.error) {
							$BU.error($BW);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_5 = this._subscription;
				let $BT = $KEY_5._observer;
				if ($KEY_5._observer === void 0) {
					return void 0;
				}
				$KEY_5._observer = void 0;
				try {
					if (typeof ($BT ? $BT.complete : void 0) === "function") {
						($BT ? $BT.complete : void 0).call($BT);
					}
				} catch (e) {}
				let $BU, $BV;
				try {
					let $BW = $KEY_5._cleanup;
					if ($BW !== void 0) {
						$KEY_5._cleanup = void 0;
						try {
							$BW();
						} catch (e_2) {}
					}
					$BU = false;
					$BV = void 0;
				} catch (e_3) {
					$BU = true;
					$BV = e_3;
				}
				if ($BU) {
					throw $BV;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_closed_getter_preserves_terminal_cleanup() {
    accepts(
        r##"class $THREAD_6_71960 {
			constructor($BW) {
				this._subscription = $BW;
			}
			next($BW) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BW);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BW) {
				let $KEY_1 = this._subscription;
				let $BX = $KEY_1._observer;
				$KEY_1._observer = void 0;
				try {
					let $BY = $KEY_1._cleanup;
					let $BZ;
					if ($BY === void 0) {
						$BZ = $BW;
					} else {
						$KEY_1._cleanup = void 0;
						try {
							$BY();
							$BZ = $BW;
						} catch (e) {
							$BZ = $BW;
						}
					}
					if ($BX) {
						if ($BX.error) {
							$BX.error($BZ);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_5 = this._subscription;
				let $BW = $KEY_5._observer;
				if ($KEY_5._observer === void 0) {
					return void 0;
				}
				$KEY_5._observer = void 0;
				try {
					if (typeof ($BW ? $BW.complete : void 0) === "function") {
						($BW ? $BW.complete : void 0).call($BW);
					}
				} catch (e) {}
				let $BX, $BY;
				try {
					let $BZ = $KEY_5._cleanup;
					if ($BZ !== void 0) {
						$KEY_5._cleanup = void 0;
						try {
							$BZ();
						} catch (e_2) {}
					}
					$BX = void 0;
					$BY = false;
				} catch (e_3) {
					$BX = e_3;
					$BY = true;
				}
				if ($BY) {
					throw $BX;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
			get closed() {
				return this._subscription._observer === void 0;
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn subscription_closed_getter_tracks_observer() {
    accepts(
        r##"class $THREAD_5_133509 {
			constructor($AL) {
				this._cleanup = void 0;
				this._observer = $AL;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $AL = this._cleanup;
					if ($AL !== void 0) {
						this._cleanup = void 0;
						try {
							$AL();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
			get closed() {
				return this._observer === void 0;
			}
		}"##,
        "subscription_class",
    );
}

#[test]
fn observer_error_call_holder_and_flag_first_completion_flags() {
    accepts(
        r##"class $THREAD_1_20151 {
			constructor($AL) {
				this._subscription = $AL;
			}
			next($AL) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($AL);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($AL) {
				let $KEY_1 = this._subscription;
				let $AM = $KEY_1._observer;
				$KEY_1._observer = void 0;
				try {
					let $AN = $KEY_1._cleanup;
					let $AO;
					if ($AN === void 0) {
						$AO = $AL;
					} else {
						$KEY_1._cleanup = void 0;
						try {
							$AN();
							$AO = $AL;
						} catch (e) {
							$AO = $AL;
						}
					}
					if ($AM) {
						if ($AM.error) {
							let $AP = [$AM.error, $AM.error.call];
							$AP[1].call($AP[0], $AM, $AO);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_3 = this._subscription;
				let $AL = $KEY_3._observer;
				if ($KEY_3._observer === void 0) {
					return void 0;
				}
				$KEY_3._observer = void 0;
				try {
					if (typeof ($AL ? $AL.complete : void 0) === "function") {
						($AL ? $AL.complete : void 0).call($AL);
					}
				} catch (e) {}
				let $AM, $AN;
				try {
					let $AO = $KEY_3._cleanup;
					if ($AO !== void 0) {
						$KEY_3._cleanup = void 0;
						try {
							$AO();
						} catch (e_2) {}
					}
					$AM = false;
					$AN = void 0;
				} catch (e_3) {
					$AM = true;
					$AN = e_3;
				}
				if ($AM) {
					throw $AN;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_flag_first_completion_keeps_unused_undefined_local() {
    accepts(
        r##"class $THREAD_9_17703 {
			constructor($BR) {
				this._subscription = $BR;
			}
			next($BR) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BR);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BR) {
				let $KEY_0 = this._subscription;
				let $BS = $KEY_0._observer;
				$KEY_0._observer = void 0;
				try {
					let $BT = $KEY_0._cleanup;
					let $BU;
					if ($BT === void 0) {
						$BU = $BR;
					} else {
						$KEY_0._cleanup = void 0;
						try {
							$BT();
							$BU = $BR;
						} catch (e) {
							$BU = $BR;
						}
					}
					if ($BS) {
						if ($BS.error) {
							let $BV = [$BS.error, $BS.error.call];
							$BV[1].call($BV[0], $BS, $BU);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_3 = this._subscription;
				let $BR = $KEY_3._observer;
				if ($KEY_3._observer === void 0) {
					return void 0;
				}
				let $BS;
				if ($BR) {
					$BS = $BR.complete;
				} else {
					$BS = void 0;
				}
				$KEY_3._observer = void 0;
				try {
					if (typeof $BS === "function") {
						$BS.call($BR);
					}
				} catch (e) {}
				let $BT, $BU;
				try {
					let $BV = $KEY_3._cleanup;
					if ($BV !== void 0) {
						$KEY_3._cleanup = void 0;
						try {
							$BV();
						} catch (e_2) {}
					}
					$BT = false;
					$BU = void 0;
				} catch (e_3) {
					$BT = true;
					$BU = e_3;
				}
				if ($BT) {
					throw $BU;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_completion_preserves_cleanup_carrier_alias() {
    accepts(
        r##"class $THREAD_2_153380 {
					constructor($BR) {
						this._subscription = $BR;
					}
					next($BR) {
						try {
							if (this._subscription._observer == null) {
								return void 0;
							}
							if (this._subscription._observer.next) {
								this._subscription._observer.next($BR);
								return void 0;
							} else {
								return void 0;
							}
						} catch (e) {
							return void 0;
						}
					}
					error($BR) {
						let $KEY_4 = this._subscription;
						let $BS = $KEY_4._observer;
						$KEY_4._observer = void 0;
						try {
							let $BT = $KEY_4._cleanup;
							let $BU;
							if ($BT === void 0) {
								$BU = $BR;
							} else {
								$KEY_4._cleanup = void 0;
								try {
									$BT();
									$BU = $BR;
								} catch (e) {
									$BU = $BR;
								}
							}
							if ($BS) {
								if ($BS.error) {
									let $BV = [$BS.error, $BS.error.call];
									$BV[1].call($BV[0], $BS, $BU);
								}
							}
						} catch (e_2) {}
					}
					complete() {
						let $KEY_8 = this._subscription;
						let $BR = $KEY_8._observer;
						if ($KEY_8._observer === void 0) {
							return void 0;
						}
						const $BS = $BR ? $BR.complete : void 0;
						$KEY_8._observer = void 0;
						try {
							if (typeof $BS === "function") {
								$BS.call($BR);
							}
						} catch (e) {}
						let $BT = $BS;
						let $BU, $BV;
						try {
							$BT = $KEY_8._cleanup;
							if ($BT !== void 0) {
								$KEY_8._cleanup = void 0;
								try {
									$BT();
								} catch (e_2) {}
							}
							$BT = 4060;
							$BU = false;
							$BV = void 0;
						} catch (e_3) {
							$BU = true;
							$BV = e_3;
						}
						if ($BU) {
							throw $BV;
						} else {
							return void 0;
						}
					}
					isComplete() {
						return this._subscription.isComplete();
					}
				}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_alpha_error_holder_preserves_error_first_flags() {
    accepts(
        r##"class $THREAD_5_98679 {
			constructor($BU) {
				this._subscription = $BU;
			}
			next($BU) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BU);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BU) {
				let $KEY_1 = this._subscription;
				let $BV = $KEY_1._observer;
				$KEY_1._observer = void 0;
				try {
					let $BW = $KEY_1._cleanup;
					let $BX;
					if ($BW === void 0) {
						$BX = $BU;
					} else {
						$KEY_1._cleanup = void 0;
						try {
							$BW();
							$BX = $BU;
						} catch (e) {
							$BX = $BU;
						}
					}
					if ($BV) {
						if ($BV.error) {
							let $BY = [$BV.error, $BV.error.call];
							$BY[1].call($BY[0], $BV, $BX);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_8 = this._subscription;
				let $BU = $KEY_8._observer;
				if ($KEY_8._observer === void 0) {
					return void 0;
				}
				$KEY_8._observer = void 0;
				$BU ? $BU.complete : void 0;
				try {
					if (typeof ($BU ? $BU.complete : void 0) === "function") {
						($BU ? $BU.complete : void 0).call($BU);
					}
				} catch (e) {}
				let $BV, $BW;
				try {
					let $BX = $KEY_8._cleanup;
					if ($BX !== void 0) {
						$KEY_8._cleanup = void 0;
						try {
							$BX();
						} catch (e_2) {}
					}
					$BV = void 0;
					$BW = false;
				} catch (e_3) {
					$BV = e_3;
					$BW = true;
				}
				if ($BW) {
					throw $BV;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_error_cleanup_uses_split_declaration() {
    accepts(
        r##"class $THREAD_2_71454 {
			constructor($BV) {
				this._subscription = $BV;
			}
			next($BV) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BV);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BV) {
				let $KEY_3 = this._subscription;
				let $BW = $KEY_3._observer;
				$KEY_3._observer = void 0;
				let $BX;
				try {
					$BX = $KEY_3._cleanup;
					let $BY;
					if ($BX === void 0) {
						$BY = $BV;
					} else {
						$KEY_3._cleanup = void 0;
						try {
							$BX();
							$BY = $BV;
						} catch (e) {
							$BY = $BV;
						}
					}
					$BX = 36565;
					if ($BW) {
						if ($BW.error) {
							let $BZ = [$BW.error, $BW.error.call];
							$BZ[1].call($BZ[0], $BW, $BY);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_1 = this._subscription;
				let $BV = $KEY_1._observer;
				if ($KEY_1._observer === void 0) {
					return void 0;
				}
				$KEY_1._observer = void 0;
				try {
					if (typeof ($BV ? $BV.complete : void 0) === "function") {
						($BV ? $BV.complete : void 0).call($BV);
					}
				} catch (e) {}
				let $BW, $BX;
				try {
					let $BY = $KEY_1._cleanup;
					if ($BY !== void 0) {
						$KEY_1._cleanup = void 0;
						try {
							$BY();
						} catch (e_2) {}
					}
					$BW = false;
					$BX = void 0;
				} catch (e_3) {
					$BW = true;
					$BX = e_3;
				}
				if ($BW) {
					throw $BX;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_error_first_completion_keeps_unused_undefined_local() {
    accepts(
        r##"class $THREAD_0_64209 {
					constructor($BX) {
						this._subscription = $BX;
					}
					next($BX) {
						try {
							if (this._subscription._observer == null) {
								return void 0;
							}
							if (this._subscription._observer.next) {
								this._subscription._observer.next($BX);
								return void 0;
							} else {
								return void 0;
							}
						} catch (e) {
							return void 0;
						}
					}
					error($BX) {
						let $KEY_1 = this._subscription;
						let $BY = $KEY_1._observer;
						$KEY_1._observer = void 0;
						try {
							let $BZ = $KEY_1._cleanup;
							let $CA;
							if ($BZ === void 0) {
								$CA = $BX;
							} else {
								$KEY_1._cleanup = void 0;
								try {
									$BZ();
									$CA = $BX;
								} catch (e) {
									$CA = $BX;
								}
							}
							if ($BY) {
								if ($BY.error) {
									let $CB = [$BY.error, $BY.error.call];
									$CB[1].call($CB[0], $BY, $CA);
								}
							}
						} catch (e_2) {}
					}
					complete() {
						let $KEY_7 = this._subscription;
						let $BX = $KEY_7._observer;
						if ($KEY_7._observer === void 0) {
							return void 0;
						}
						let $BY;
						if ($BX) {
							$BY = $BX.complete;
						} else {
							$BY = void 0;
						}
						$KEY_7._observer = void 0;
						try {
							if (typeof $BY === "function") {
								$BY.call($BX);
							}
						} catch (e) {}
						let $BZ, $CA;
						try {
							let $CB = $KEY_7._cleanup;
							if ($CB !== void 0) {
								$KEY_7._cleanup = void 0;
								try {
									$CB();
								} catch (e_2) {}
							}
							$BZ = void 0;
							$CA = false;
						} catch (e_3) {
							$BZ = e_3;
							$CA = true;
						}
						if ($CA) {
							throw $BZ;
						} else {
							return void 0;
						}
					}
					isComplete() {
						return this._subscription.isComplete();
					}
				}"##,
        "subscription_observer_class",
    );
}

#[test]
fn observer_alpha_error_holder_preserves_flag_first_flags() {
    accepts(
        r##"class $THREAD_1_88236 {
					constructor($AF) {
						this._subscription = $AF;
					}
					next($AF) {
						try {
							if (this._subscription._observer == null) {
								return void 0;
							}
							if (this._subscription._observer.next) {
								this._subscription._observer.next($AF);
								return void 0;
							} else {
								return void 0;
							}
						} catch (e) {
							return void 0;
						}
					}
					error($AF) {
						let $KEY_2 = this._subscription;
						let $AG = $KEY_2._observer;
						$KEY_2._observer = void 0;
						try {
							let $AH = $KEY_2._cleanup;
							let $AI;
							if ($AH === void 0) {
								$AI = $AF;
							} else {
								$KEY_2._cleanup = void 0;
								try {
									$AH();
									$AI = $AF;
								} catch (e) {
									$AI = $AF;
								}
							}
							if ($AG) {
								if ($AG.error) {
									let $AJ = [$AG.error, $AG.error.call];
									$AJ[1].call($AJ[0], $AG, $AI);
								}
							}
						} catch (e_2) {}
					}
					complete() {
						let $KEY_1 = this._subscription;
						let $AF = $KEY_1._observer;
						if ($KEY_1._observer === void 0) {
							return void 0;
						}
						$KEY_1._observer = void 0;
						$AF ? $AF.complete : void 0;
						try {
							if (typeof ($AF ? $AF.complete : void 0) === "function") {
								($AF ? $AF.complete : void 0).call($AF);
							}
						} catch (e) {}
						let $AG, $AH;
						try {
							let $AI = $KEY_1._cleanup;
							if ($AI !== void 0) {
								$KEY_1._cleanup = void 0;
								try {
									$AI();
								} catch (e_2) {}
							}
							$AG = false;
							$AH = void 0;
						} catch (e_3) {
							$AG = true;
							$AH = e_3;
						}
						if ($AG) {
							throw $AH;
						} else {
							return void 0;
						}
					}
					isComplete() {
						return this._subscription.isComplete();
					}
				}"##,
        "subscription_observer_class",
    );
}

// Exact class + seven setup statements + stable alias from pinned inputs.
const SETUP_SUBSCRIPTION_BT: &str = r##"class $THREAD_8_57126 {
			constructor($BT) {
				this._cleanup = void 0;
				this._observer = $BT;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $BT = this._cleanup;
					if ($BT !== void 0) {
						this._cleanup = void 0;
						try {
							$BT();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
		}
		let $P = { key: "closed" };
		function $THREAD_4_69994() {
			"use strict";
			return this._observer === void 0;
		}
		$P.get = $THREAD_4_69994;
		$P = [$P];
		let i = 0;
		for (; i < $P.length; i++) {
			$P[i].enumerable = $P[i].enumerable || false;
			$P[i].configurable = true;
			if ("value" in $P[i]) {
				$P[i].writable = true;
			}
			let $Q;
			if (typeof $P[i].key != "object" ? typeof $P[i].key != "object" : !$P[i].key) {
				$Q = $P[i].key;
			} else if ($P[i].key[Symbol.toPrimitive] !== void 0) {
				$Q = $P[i].key[Symbol.toPrimitive].call($P[i].key, "string");
				if (typeof $Q == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$Q = String($P[i].key);
			}
			Object.defineProperty($THREAD_8_57126.prototype, typeof $Q == "symbol" ? $Q : $Q + "", $P[i]);
		}
		Object.defineProperty($THREAD_8_57126, "prototype", { writable: false });
		let $KEY_419 = $THREAD_8_57126;
new $KEY_419({});"##;

#[test]
fn subscription_bt_parameter_preserves_setup_lifecycle() {
    accepts(SETUP_SUBSCRIPTION_BT, "subscription_class");
}
const SETUP_OBSERVER_BT: &str = r##"class $THREAD_0_94586 {
			constructor($BT) {
				this._subscription = $BT;
			}
			next($BT) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BT);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BT) {
				let $KEY_4 = this._subscription;
				let $BU = $KEY_4._observer;
				$KEY_4._observer = void 0;
				try {
					let $BV = $KEY_4._cleanup;
					let $BW;
					if ($BV === void 0) {
						$BW = $BT;
					} else {
						$KEY_4._cleanup = void 0;
						try {
							$BV();
							$BW = $BT;
						} catch (e) {
							$BW = $BT;
						}
					}
					if ($BU) {
						if ($BU.error) {
							$BU.error($BW);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_5 = this._subscription;
				let $BT = $KEY_5._observer;
				if ($KEY_5._observer === void 0) {
					return void 0;
				}
				$KEY_5._observer = void 0;
				try {
					if (typeof ($BT ? $BT.complete : void 0) === "function") {
						($BT ? $BT.complete : void 0).call($BT);
					}
				} catch (e) {}
				let $BU, $BV;
				try {
					let $BW = $KEY_5._cleanup;
					if ($BW !== void 0) {
						$KEY_5._cleanup = void 0;
						try {
							$BW();
						} catch (e_2) {}
					}
					$BU = false;
					$BV = void 0;
				} catch (e_3) {
					$BU = true;
					$BV = e_3;
				}
				if ($BU) {
					throw $BV;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $R = { key: "closed" };
		function $THREAD_9_149346() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$R.get = $THREAD_9_149346;
		$R = [$R];
		let j = 0;
		for (; j < $R.length; j++) {
			$R[j].enumerable = $R[j].enumerable || false;
			$R[j].configurable = true;
			if ("value" in $R[j]) {
				$R[j].writable = true;
			}
			let $S;
			if (typeof $R[j].key != "object" ? typeof $R[j].key != "object" : !$R[j].key) {
				$S = $R[j].key;
			} else if ($R[j].key[Symbol.toPrimitive] !== void 0) {
				$S = $R[j].key[Symbol.toPrimitive].call($R[j].key, "string");
				if (typeof $S == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$S = String($R[j].key);
			}
			Object.defineProperty($THREAD_0_94586.prototype, typeof $S == "symbol" ? $S : $S + "", $R[j]);
		}
		Object.defineProperty($THREAD_0_94586, "prototype", { writable: false });
		let $KEY_100 = $THREAD_0_94586;
new $KEY_100({});"##;

#[test]
fn observer_bt_parameter_preserves_setup_lifecycle() {
    accepts(SETUP_OBSERVER_BT, "subscription_observer_class");
}
const SETUP_SUBSCRIPTION_AM: &str = r##"class $THREAD_9_119835 {
			constructor($AM) {
				this._cleanup = void 0;
				this._observer = $AM;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $AM = this._cleanup;
					if ($AM !== void 0) {
						this._cleanup = void 0;
						try {
							$AM();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
		}
		let $T = { key: "closed" };
		function $THREAD_0_132142() {
			"use strict";
			return this._observer === void 0;
		}
		$T.get = $THREAD_0_132142;
		$T = [$T];
		let i = 0;
		for (; i < $T.length; i++) {
			$T[i].enumerable = $T[i].enumerable || false;
			$T[i].configurable = true;
			if ("value" in $T[i]) {
				$T[i].writable = true;
			}
			let $U;
			if (typeof $T[i].key != "object" || !$T[i].key) {
				$U = $T[i].key;
			} else if ($T[i].key[Symbol.toPrimitive] !== void 0) {
				let $V = [$T[i].key[Symbol.toPrimitive], $T[i].key[Symbol.toPrimitive].call];
				$U = $V[1].call($V[0], $T[i].key, "string");
				if (typeof $U == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$U = String($T[i].key);
			}
			Object.defineProperty($THREAD_9_119835.prototype, typeof $U == "symbol" ? $U : String($U), $T[i]);
		}
		Object.defineProperty($THREAD_9_119835, "prototype", { writable: false });
		let $KEY_234 = $THREAD_9_119835;
new $KEY_234({});"##;

#[test]
fn subscription_am_parameter_preserves_setup_lifecycle() {
    accepts(SETUP_SUBSCRIPTION_AM, "subscription_class");
}
const SETUP_OBSERVER_AM: &str = r##"class $THREAD_4_68013 {
			constructor($AM) {
				this._subscription = $AM;
			}
			next($AM) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($AM);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($AM) {
				let $KEY_1 = this._subscription;
				let $AN = $KEY_1._observer;
				$KEY_1._observer = void 0;
				try {
					let $AO = $KEY_1._cleanup;
					let $AP;
					if ($AO === void 0) {
						$AP = $AM;
					} else {
						$KEY_1._cleanup = void 0;
						try {
							$AO();
							$AP = $AM;
						} catch (e) {
							$AP = $AM;
						}
					}
					if ($AN) {
						if ($AN.error) {
							let $AQ = [$AN.error, $AN.error.call];
							$AQ[1].call($AQ[0], $AN, $AP);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_4 = this._subscription;
				let $AM = $KEY_4._observer;
				if ($KEY_4._observer === void 0) {
					return void 0;
				}
				$KEY_4._observer = void 0;
				try {
					if (typeof ($AM ? $AM.complete : void 0) === "function") {
						($AM ? $AM.complete : void 0).call($AM);
					}
				} catch (e) {}
				let $AN, $AO;
				try {
					let $AP = $KEY_4._cleanup;
					if ($AP !== void 0) {
						$KEY_4._cleanup = void 0;
						try {
							$AP();
						} catch (e_2) {}
					}
					$AN = void 0;
					$AO = false;
				} catch (e_3) {
					$AN = e_3;
					$AO = true;
				}
				if ($AO) {
					throw $AN;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $W = { key: "closed" };
		function $THREAD_2_132048() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$W.get = $THREAD_2_132048;
		$W = [$W];
		let j = 0;
		for (; j < $W.length; j++) {
			$W[j].enumerable = $W[j].enumerable || false;
			$W[j].configurable = true;
			if ("value" in $W[j]) {
				$W[j].writable = true;
			}
			let $X;
			if (typeof $W[j].key != "object" || !$W[j].key) {
				$X = $W[j].key;
			} else if ($W[j].key[Symbol.toPrimitive] !== void 0) {
				let $Y = [$W[j].key[Symbol.toPrimitive], $W[j].key[Symbol.toPrimitive].call];
				$X = $Y[1].call($Y[0], $W[j].key, "string");
				if (typeof $X == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$X = String($W[j].key);
			}
			Object.defineProperty($THREAD_4_68013.prototype, typeof $X == "symbol" ? $X : String($X), $W[j]);
		}
		Object.defineProperty($THREAD_4_68013, "prototype", { writable: false });
		let $KEY_86 = $THREAD_4_68013;
new $KEY_86({});"##;

#[test]
fn observer_am_parameter_preserves_setup_lifecycle() {
    accepts(SETUP_OBSERVER_AM, "subscription_observer_class");
}
const SETUP_SUBSCRIPTION_CB: &str = r##"class $THREAD_1_57124 {
					constructor($CB) {
						this._cleanup = void 0;
						this._observer = $CB;
					}
					unsubscribe() {
						if (this.isComplete()) {
							return void 0;
						} else {
							this._observer = void 0;
							let $CB = this._cleanup;
							if ($CB !== void 0) {
								this._cleanup = void 0;
								try {
									$CB();
								} catch (e) {}
							}
							return void 0;
						}
					}
					isComplete() {
						return this._observer === void 0;
					}
				}
				let $O = { key: "closed" };
				function $THREAD_0_81() {
					"use strict";
					return this._observer === void 0;
				}
				$O.get = $THREAD_0_81;
				$O = [$O];
				let i = 0;
				for (; i < $O.length; i++) {
					$O[i].enumerable = $O[i].enumerable || false;
					$O[i].configurable = true;
					if ("value" in $O[i]) {
						$O[i].writable = true;
					}
					let $P;
					if (typeof $O[i].key != "object" ? typeof $O[i].key != "object" : !$O[i].key) {
						$P = $O[i].key;
					} else if ($O[i].key[Symbol.toPrimitive] !== void 0) {
						let $Q = [$O[i].key[Symbol.toPrimitive], $O[i].key[Symbol.toPrimitive].call];
						$P = $Q[1].call($Q[0], $O[i].key, "string");
						if (typeof $P == "object") {
							throw new TypeError("@@toPrimitive must return a primitive value.");
						}
					} else {
						$P = String($O[i].key);
					}
					Object.defineProperty($THREAD_1_57124.prototype, typeof $P == "symbol" ? $P : String($P), $O[i]);
				}
				Object.defineProperty($THREAD_1_57124, "prototype", { writable: false });
				let $KEY_100 = $THREAD_1_57124;
new $KEY_100({});"##;

#[test]
fn subscription_cb_parameter_preserves_setup_lifecycle() {
    accepts(SETUP_SUBSCRIPTION_CB, "subscription_class");
}
const SETUP_OBSERVER_CB: &str = r##"class $THREAD_9_70251 {
					constructor($CB) {
						this._subscription = $CB;
					}
					next($CB) {
						try {
							if (this._subscription._observer == null) {
								return void 0;
							}
							if (this._subscription._observer.next) {
								this._subscription._observer.next($CB);
								return void 0;
							} else {
								return void 0;
							}
						} catch (e) {
							return void 0;
						}
					}
					error($CB) {
						let $KEY_4 = this._subscription;
						let $CC = $KEY_4._observer;
						$KEY_4._observer = void 0;
						try {
							let $CD = $KEY_4._cleanup;
							let $CE;
							if ($CD === void 0) {
								$CE = $CB;
							} else {
								$KEY_4._cleanup = void 0;
								try {
									$CD();
									$CE = $CB;
								} catch (e) {
									$CE = $CB;
								}
							}
							if ($CC) {
								if ($CC.error) {
									let $CF = [$CC.error, $CC.error.call];
									$CF[1].call($CF[0], $CC, $CE);
								}
							}
						} catch (e_2) {}
					}
					complete() {
						let $KEY_2 = this._subscription;
						let $CB = $KEY_2._observer;
						if ($KEY_2._observer === void 0) {
							return void 0;
						}
						$KEY_2._observer = void 0;
						try {
							if (typeof ($CB ? $CB.complete : void 0) === "function") {
								($CB ? $CB.complete : void 0).call($CB);
							}
						} catch (e) {}
						let $CC, $CD;
						try {
							let $CE = $KEY_2._cleanup;
							if ($CE !== void 0) {
								$KEY_2._cleanup = void 0;
								try {
									$CE();
								} catch (e_2) {}
							}
							$CC = false;
							$CD = void 0;
						} catch (e_3) {
							$CC = true;
							$CD = e_3;
						}
						if ($CC) {
							throw $CD;
						} else {
							return void 0;
						}
					}
					isComplete() {
						return this._subscription.isComplete();
					}
				}
				let $R = { key: "closed" };
				function $THREAD_2_51604() {
					"use strict";
					return this._subscription._observer === void 0;
				}
				$R.get = $THREAD_2_51604;
				$R = [$R];
				let j = 0;
				for (; j < $R.length; j++) {
					$R[j].enumerable = $R[j].enumerable || false;
					$R[j].configurable = true;
					if ("value" in $R[j]) {
						$R[j].writable = true;
					}
					let $S;
					if (typeof $R[j].key != "object" ? typeof $R[j].key != "object" : !$R[j].key) {
						$S = $R[j].key;
					} else if ($R[j].key[Symbol.toPrimitive] !== void 0) {
						let $T = [$R[j].key[Symbol.toPrimitive], $R[j].key[Symbol.toPrimitive].call];
						$S = $T[1].call($T[0], $R[j].key, "string");
						if (typeof $S == "object") {
							throw new TypeError("@@toPrimitive must return a primitive value.");
						}
					} else {
						$S = String($R[j].key);
					}
					Object.defineProperty($THREAD_9_70251.prototype, typeof $S == "symbol" ? $S : String($S), $R[j]);
				}
				Object.defineProperty($THREAD_9_70251, "prototype", { writable: false });
				let $KEY_254 = $THREAD_9_70251;
new $KEY_254({});"##;

#[test]
fn observer_cb_parameter_preserves_setup_lifecycle() {
    accepts(SETUP_OBSERVER_CB, "subscription_observer_class");
}

#[test]
#[ignore = "requires the pinned local integration input cache"]
fn pinned_full_cached_inputs_prove_both_native_class_spans() {
    use sha2::{Digest, Sha256};
    let inputs: &[(&str, &str, &[(u32, u32, &str)])] = &[
        ("/corpus/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", "bdca471898bfa4d2c022b5ae483199992a44ed4114c0277ade8d6159a177b500", &[(155632, 156084, "subscription_class"), (157037, 158630, "subscription_observer_class")]),
        ("/corpus/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", "970d9b01a364c3d7420332a160da3103a17c1d6baa0e3dc253d04fe20b7cac77", &[(107039, 107492, "subscription_class"), (108488, 110142, "subscription_observer_class")]),
        ("/corpus/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", "115a025ee44eee8fe6c2cbe1818ad9cf25b223ae5e47c7ce5ebcbcdb0bdb4af9", &[(113040, 113538, "subscription_class"), (114615, 116431, "subscription_observer_class")]),
    ];
    for (path, pin, expected) in inputs {
        let bytes = std::fs::read(path).expect("pinned cached input");
        assert_eq!(
            Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            *pin
        );
        let source = std::str::from_utf8(&bytes).unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
        assert!(parsed.errors.is_empty());
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let (scoping, nodes) = semantic.into_scoping_and_nodes();
        let model = Model::new(
            source,
            &parsed.program,
            &scoping,
            &nodes,
            ProgramPropertiesLimits::default(),
        )
        .unwrap();
        let found = claims(&model);
        let actual: Vec<_> = found
            .iter()
            .map(|claim| {
                let FinderClaim::ClassBinding {
                    class_span, role, ..
                } = claim
                else {
                    panic!("function claim")
                };
                (class_span.start, class_span.end, *role)
            })
            .collect();
        assert_eq!(&actual, expected, "{path}");
        eprintln!("full cached static proof: {path}: {actual:?}");
    }
}

fn changed_setup(from: &str, to: &str) -> String {
    let start = SETUP_SUBSCRIPTION_BT.find("let $P =").unwrap();
    let (class, setup) = SETUP_SUBSCRIPTION_BT.split_at(start);
    assert!(setup.contains(from), "missing mutation: {from}");
    format!("{class}{}", setup.replace(from, to))
}

#[test]
fn setup_rejects_foreign_getter_descriptor_key_receiver_and_overwrites() {
    for (from, to) in [
        (
            "return this._observer === void 0",
            "return other._observer === void 0",
        ),
        (
            "return this._observer === void 0",
            "return this._subscription._observer === void 0",
        ),
        (
            "return this._observer === void 0",
            "return this._observer !== void 0",
        ),
        (
            "function $THREAD_4_69994()",
            "function $THREAD_4_69994(unused)",
        ),
        ("{ key: \"closed\" }", "{ key: \"unsubscribe\" }"),
        ("{ key: \"closed\" }", "{ key: \"closed\", value: 0 }"),
        ("{ key: \"closed\" }", "{ key: \"closed\", set: foreign }"),
        ("$P.get = $THREAD_4_69994", "$P.get = foreignGetter"),
        ("$P = [$P]", "$P = [foreignDescriptor]"),
        ("$THREAD_8_57126.prototype", "Foreign.prototype"),
        ("$Q + \"\", $P[i]", "$Q + \"\", foreignDescriptor"),
        ("let i = 0", "let i = 1"),
        ("i++", "i--"),
        (
            "$P[i].configurable = true",
            "$P[i].configurable = true; $P[i].key = \"unsubscribe\"",
        ),
        (
            "$P[i].configurable = true",
            "$P[i].configurable = true; $P[i].get = foreignGetter",
        ),
        (
            "$P[i].configurable = true",
            "$P[i].configurable = true; $P[i][unknown] = 0",
        ),
        ("{ writable: false }", "{ writable: true }"),
        ("{ writable: false }", "{ writable: false, value: {} }"),
        ("\"prototype\", { writable", "\"unsubscribe\", { writable"),
        (
            "function $THREAD_4_69994()",
            "async function $THREAD_4_69994()",
        ),
    ] {
        refuses(&changed_setup(from, to));
    }
}

#[test]
fn setup_rejects_later_mutations_escapes_shadowed_globals_and_donors() {
    for extra in [
        "$P[0].key = 'unsubscribe';",
        "$P[0].get = foreign;",
        "$P = other;",
        "$THREAD_4_69994 = foreign;",
        "escape($P);",
        "escape($THREAD_4_69994);",
        "$KEY_419 = Other;",
        "$KEY_419.prototype.unsubscribe = 0;",
        "$THREAD_8_57126.prototype[unknown] = 0;",
        "delete $KEY_419.prototype.unsubscribe;",
        "Object.defineProperty($KEY_419.prototype, 'closed', foreign);",
    ] {
        refuses(&format!("{SETUP_SUBSCRIPTION_BT}\n{extra}"));
    }
    refuses(&format!("let Object = foreign;\n{SETUP_SUBSCRIPTION_BT}"));
    refuses(&format!("let String = foreign;\n{SETUP_SUBSCRIPTION_BT}"));
    refuses(&format!("let Symbol = foreign;\n{SETUP_SUBSCRIPTION_BT}"));
    refuses(&changed_setup(
        "return this._observer === void 0",
        "function donor() { return this._observer === void 0; } return false",
    ));
    refuses(&changed_setup(
        "return this._observer === void 0",
        "return (() => this._observer === void 0)()",
    ));
    // A correct sibling getter cannot substitute for the function attached here.
    refuses(&format!(
        "{}\nfunction siblingGetter() {{ return this._observer === void 0; }}",
        changed_setup("return this._observer === void 0", "return false")
    ));
}

#[test]
fn readonly_prototype_is_independently_exact_and_global() {
    accepts(&format!("{SUBSCRIPTION}\nObject.defineProperty(Subscription, 'prototype', {{ writable: false }});"), "subscription_class");
    for descriptor in [
        "{writable:true}",
        "{writable:false,value:0}",
        "{}",
        "unknown",
    ] {
        refuses(&format!(
            "{SUBSCRIPTION}\nObject.defineProperty(Subscription, 'prototype', {descriptor});"
        ));
    }
    refuses(&format!("let Object = foreign; {SUBSCRIPTION}\nObject.defineProperty(Subscription, 'prototype', {{writable:false}});"));
}

const DIRECT_INSTALLER_SOURCE: &str = include_str!("../../../tests/fixtures/program/thread-lifecycle-direct.js");

fn native_sites(source: &str) -> Vec<(u32, u32, &'static str)> {
    prove(source).into_iter().map(|claim| {
        let FinderClaim::ClassBinding { class_span, role, .. } = claim else {
            panic!("lifecycle proof must not produce function gains")
        };
        (class_span.start - PREFIX.len() as u32, class_span.end - PREFIX.len() as u32, role)
    }).collect()
}

#[test]
fn current_direct_installer_source_installer_retains_bootstrap_aliases_and_consumers() {
    assert_eq!(native_sites(DIRECT_INSTALLER_SOURCE), vec![
        (140159, 140657, "subscription_class"),
        (141225, 143041, "subscription_observer_class"),
    ]);
}

#[test]
fn direct_installer_refuses_mutation_escape_wrong_receiver_and_flags() {
    let expected = (140159, 140657, "subscription_class");
    for (old, new) in [
        ("let $Q = { key: \"closed\" };", "let $Q = { key: \"unsubscribe\" };"),
        ("$Q.get = $THREAD_0_48614;", "$Q.get = foreign;"),
        ("$Q = [$Q];", "$Q = [foreign];"),
        ("$Q[i].configurable = true;", "$Q[i].configurable = false;"),
        ("$Q[i].configurable = true;", "$Q[i].configurable = true; $Q[i].key = 'other';"),
        ("$Q[i].key, $Q[i]", "$Q[i].key, foreign"),
        ("$THREAD_1_67028.prototype, $Q[i].key", "Foreign.prototype, $Q[i].key"),
        ("$Q.get = $THREAD_0_48614;", "$Q.get = $THREAD_0_48614; escape($Q);"),
        ("let $KEY_284 = $THREAD_1_67028;", "let $KEY_284 = $THREAD_1_67028; $KEY_284 = foreign;"),
    ] {
        assert!(DIRECT_INSTALLER_SOURCE.contains(old), "missing mutation: {old}");
        let changed = DIRECT_INSTALLER_SOURCE.replacen(old, new, 1);
        // native_sites always parses and constructs a valid Model before refusal.
        // Compare roles because mutations can change subsequent UTF8 offsets.
        assert!(native_sites(&changed).iter().all(|site| site.2 != expected.2), "accepted mutation: {old} -> {new}");
    }
}

const CLOSED_STRING_SOURCE: &str = include_str!("../../../tests/fixtures/program/thread-lifecycle-finite.js");

#[test]
fn current_closed_string_closed_string_proves_coercion_and_symbol_arms_unreachable() {
    assert!(native_sites(CLOSED_STRING_SOURCE).contains(&(155576, 156026, "subscription_class")));
    // Scratch writes in these arms are safe only because the whole installer
    // proves the key remains the private literal string, including its order.
    for (old, new) in [
        ("let $N = { key: \"closed\" };", "let $N = { key: dynamicKey };"),
        ("$O = $N[i].key;", "$O = Symbol('closed');"),
        ("$O = $N[i].key;", "$O = $N[i].key; $O = Symbol('closed');"),
        ("$N[i].configurable = true;", "$N[i].configurable = true; escape($N);"),
        ("$O = $O + \"\";", "$O = 'unsubscribe';"),
        ("$THREAD_0_334.prototype, $O", "Foreign.prototype, $O"),
    ] {
        assert!(CLOSED_STRING_SOURCE.contains(old), "missing mutation: {old}");
        let changed = CLOSED_STRING_SOURCE.replacen(old, new, 1);
        assert!(native_sites(&changed).iter().all(|site| site.2 != "subscription_class"), "accepted mutation: {old}");
    }
}

const SEPARATE_LIST: &str = include_str!("../../../tests/fixtures/program/thread-lifecycle-separate-list.js");
const SAVED_SYMBOL: &str = include_str!("../../../tests/fixtures/program/thread-lifecycle-saved-symbol.js");
const WHILE_INSTALLER: &str = include_str!("../../../tests/fixtures/program/thread-lifecycle-while-installer.js");
const TUPLE_KEY: &str = include_str!("../../../tests/fixtures/program/thread-lifecycle-tuple-key.js");

#[test]
fn current_installer_variants_keep_separate_class_identities_and_consumers() {
    for source in [SEPARATE_LIST, SAVED_SYMBOL, WHILE_INSTALLER, TUPLE_KEY] {
        let roles: Vec<_> = native_sites(source).into_iter().map(|site| site.2).collect();
        assert_eq!(roles, ["subscription_class", "subscription_observer_class"]);
    }
}

#[test]
fn variant_installers_refuse_foreign_lists_receivers_indices_and_effect_order() {
    for (source, role, old, new) in [
        (SEPARATE_LIST, "subscription_class", "let $KEY_4 = [$R];", "let $KEY_4 = [foreign];"),
        (SEPARATE_LIST, "subscription_class", "$THREAD_9_130851.prototype,", "$THREAD_3_39152.prototype,"),
        (SEPARATE_LIST, "subscription_class", "let $KEY_4 = [$R];", "let $KEY_4 = [$R]; escape($KEY_4);"),
        (SAVED_SYMBOL, "subscription_observer_class", "$X ? $Y : String($Y)", "$X ? $Y : String($W)"),
        (SAVED_SYMBOL, "subscription_observer_class", "$X = typeof $W[j].key == \"symbol\";", "$X = true;"),
        (WHILE_INSTALLER, "subscription_class", "++$S;", "--$S;"),
        (WHILE_INSTALLER, "subscription_class", "$THREAD_7_59971.prototype,", "Foreign.prototype,"),
        (WHILE_INSTALLER, "subscription_class", "$R[$S].configurable = true;", "++$S; $R[$S].configurable = true;"),
        (TUPLE_KEY, "subscription_class", "$M = [$M, $M];", "$M = [$M, foreign];"),
        (TUPLE_KEY, "subscription_class", "$M[0] + \"\"", "$M[1] + \"\""),
    ] {
        assert!(source.contains(old), "missing mutation: {old}");
        let changed = source.replacen(old, new, 1);
        assert!(native_sites(&changed).iter().all(|site| site.2 != role), "accepted mutation: {old}");
    }
}
