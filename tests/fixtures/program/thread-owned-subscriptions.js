function bootstrap() {
class $THREAD_901_1 {
    constructor(observer) { this._cleanup = void 0; this._observer = observer; }
    unsubscribe() {
        if (this.isComplete()) { return void 0; } else {
            this._observer = void 0;
            let cleanup = this._cleanup;
            if (cleanup !== void 0) {
                this._cleanup = void 0;
                try { cleanup(); } catch (caught) {}
            }
            return void 0;
        }
    }
    isComplete() { return this._observer === void 0; }
    get closed() { return this._observer === void 0; }
}
class $THREAD_901_2 {
    constructor(subscription) { this._subscription = subscription; }
    next(input) {
        try {
            if (this._subscription._observer == null) { return void 0; }
            if (this._subscription._observer.next) {
                this._subscription._observer.next(input); return void 0;
            } else { return void 0; }
        } catch (caught) { return void 0; }
    }
    error(input) {
        let sub = this._subscription;
        let observer = sub._observer;
        sub._observer = void 0;
        try {
            let cleanup = sub._cleanup;
            let value;
            if (cleanup === void 0) { value = input; } else {
                sub._cleanup = void 0;
                try { cleanup(); value = input; } catch (caught) { value = input; }
            }
            if (observer) { if (observer.error) { observer.error(value); } }
        } catch (caught) {}
    }
    complete() {
        let sub = this._subscription;
        let observer = sub._observer;
        if (sub._observer === void 0) { return void 0; }
        sub._observer = void 0;
        try {
            if (typeof (observer ? observer.complete : void 0) === "function") {
                (observer ? observer.complete : void 0).call(observer);
            }
        } catch (caught) {}
        let failure, failed;
        try {
            let cleanup = sub._cleanup;
            if (cleanup !== void 0) {
                sub._cleanup = void 0;
                try { cleanup(); } catch (caught) {}
            }
            failure = void 0; failed = false;
        } catch (caught) { failure = caught; failed = true; }
        if (failed) { throw failure; } else { return void 0; }
    }
    isComplete() { return this._subscription.isComplete(); }
    get closed() { return this._subscription._observer === void 0; }
}
const subscriptionAlias = $THREAD_901_1;
const observerAlias = $THREAD_901_2;
new subscriptionAlias({}); new observerAlias(new $THREAD_901_1({}));
}
bootstrap();
