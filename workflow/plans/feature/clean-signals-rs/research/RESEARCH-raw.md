## reactive-graph-standalone-api

**Summary:** Leptos 0.8.x reactive system is usable standalone via `reactive_graph` ^0.2.14 (paired with leptos ^0.8.19+) or by using `leptos` with default-features disabled. The `effects` feature flag must be explicitly enabled for effects to run. Owner API includes Owner::new(), Owner::with(), Owner::cleanup(), and Owner::on_cleanup(). The arena-allocated RwSignal is Send+Sync when T: Send+Sync (via RwSignal::new()); RwSignal::new_local() is !Send. any_spawner provides Executor::init_tokio() and Executor::init_wasm_bindgen() (version 0.3.0+). Effect::new requires an active Owner context and spawns locally; Effect::new_sync runs on any thread. StoredValue is a non-reactive, Copy handle; ArcRwSignal is the reference-counted alternative.

**Caveats:** Owner::as_current() does not exist in the API; use Owner::with() instead. Executor::tick() is not part of any_spawner API (this may have been from a different executor abstraction or misremembered from Tokio's runtime). The effects feature flag is tied to rendering modes in web contexts (csr/hydrate enable it automatically), but for standalone library tests, it must be manually enabled as a feature. The LocalStorage vs SyncStorage distinction is automatic based on which constructor (new() vs new_local()) is used—no explicit feature flag selection is needed. RwSignal Send+Sync traits are correctly implemented when T: Send+Sync only for the default arena storage; custom storage types may have different bounds."

**Verified claims:**
- reactive_graph ^0.2.14 pairs with Leptos ^0.8.19 and later versions (0.8.20+)
- Leptos 0.8.x has NO default features enabled; exactly one of csr/ssr/hydrate must be explicitly chosen
- Feature flag 'effects' is NOT automatically enabled; it must be explicitly enabled via csr or hydrate feature, or independently for standalone tests
- Owner::new() creates a new Owner; Owner::with() executes closure as current owner; Owner::cleanup() runs cleanups, child cleanups, and arena disposal
- Owner::on_cleanup(fun) registers a function to run on next owner cleanup; it's the standard on_cleanup pattern for outside-component use
- any_spawner 0.3.0 provides Executor::init_tokio() and Executor::init_wasm_bindgen() exactly; no Executor::tick() method exists
- Effect::new spawns a task on the local thread (using spawn_local), requires active Owner context, and DOES run without explicit tracking scope if Effect feature is enabled
- Effect::new_sync is the Send+Sync variant; Effect::new is LocalStorage (spawn_local) variant
- RwSignal::new(value) where T: Send + Sync + 'static is Send + Sync; RwSignal::new_local() is !Send and panics if accessed from other threads
- StoredValue is a non-reactive, Copy handle for storing values; does NOT trigger effects on access/mutation
- ArcRwSignal is reference-counted alternative to arena RwSignal; useful when signal lifetime exceeds Owner scope
- Owner::with() is the standard guard mechanism (not Owner::as_current()); it executes a closure with owner as current
- For standalone use in #[tokio::test], declare dependency 'reactive_graph = "0.2.14"' and feature 'effects = ["reactive_graph/effects"]' in Cargo.toml

## async-trait-dyn-strategy

**Summary:** Native async fn in trait (Rust 1.75+) is NOT dyn-compatible; traits using async fn cannot be used with trait objects. Production patterns: (1) #[async_trait] with ?Send cfg-gated for wasm (async-trait crate, heap allocation), (2) trait_variant for explicit Send/!Send variants (zero allocation), (3) generic-only design (best if viable), (4) dynosaur (experimental Rust 1.75+, zero-cost dyn async). Ecosystem crates avoid dyn dispatch for async: Leptos uses cfg!-gated executors; Gloo uses generic futures. For a framework seeking Arc<dyn UseCase>, recommend trait_variant + #[async_trait] with cfg_attr for Repository (native requires Send, wasm doesn't), or prefer generic dispatch if polymorphism via dyn isn't required. Migrate to dynosaur in 2025+ when stable.

**Caveats:** none

**Verified claims:**
- Native async fn in trait (stabilized Rust 1.75) cannot be used with dyn Trait; async fn in traits lack object-safety.
- async_trait crate (#[async_trait] macro) is the production-standard workaround for dyn async traits; uses Pin<Box<dyn Future + Send>> by default.
- async_trait supports #[async_trait(?Send)] to remove Send bounds, enabling wasm/single-threaded executors; must be applied to both trait and impl blocks.
- trait_variant crate (rust-lang org) creates explicit Send/!Send trait variants via #[trait_variant::make(SendVariant: Send)].
- dynosaur is an experimental proc-macro (Rust 1.75+) that enables zero-cost dyn dispatch on async fn traits via #[dynosaur::dynosaur(DynTrait = dyn(box) Trait)].
- maybe-sync crate implements MaybeSend and MaybeSync trait aliases that conditionally resolve to Send/Sync or empty traits, enabling single-codebase native/wasm support.
- Leptos handles wasm/native async divergence via cfg-gated executors (spawn_local for wasm, tokio spawn for native), not dyn traits; avoids async trait objects.
- Gloo crate avoids dyn trait dispatch for async; provides generic future-based APIs (e.g., gloo_timers::future::TimeoutFuture) that work on both wasm and native without Send requirements.
- For Arc<dyn UseCase> supporting both native and wasm, trait_variant + #[async_trait] with cfg_attr on Repository (native: Send, wasm: ?Send) is recommended pattern; generic dispatch preferred if polymorphism not required.

## dart-test-suite-spec

**Summary:** The clean_signals test suite contains 32 tests across 5 files that pin critical behaviors for a Rust port. Key abstractions include Result (Success/Failed with map/flatMap/fold), UseCase (execute wraps thrown Failures as Failed, unknown errors as UnexpectedFailure), ActivityTracker (ref-counted loading), RetryPolicy (exponential backoff with isRetryable checks), and Controller (run/runInto/watch/dispose with failures broadcast stream). Critical semantics: isLoading transitions with fine-grained await ordering; AsyncDataReloading preserves stale data while reloading; failures emit only final result (not intermediates during retries); dispose runs onDispose callbacks in reverse order (idempotent), cancels watch subscriptions, and guards post-dispose signal writes; StreamUseCase catches stream errors and emits as trailing Failed event with immediate cancellation.

**Caveats:** The specification is extracted from test code only; runtime behavior (e.g., exact timing of async transitions, signal reactivity) is inferred from test assertions but not explicitly verified against implementation source code. Integration tests (app_test.dart) depend on FakeTeamApi and FakeProfileApi which are not analyzed here—their behavior is inferred from test usage. The Rust port must ensure equivalent semantics for: (1) Result type algebra (fold, map, flatMap, pattern matching); (2) UseCase error wrapping rules; (3) StreamUseCase stream error handling and immediate cancellation; (4) ActivityTracker ref-counting with post-dispose guard; (5) RetryPolicy backoff formula and retryable predicate; (6) Controller's isLoading/failures async ordering and dispose LIFO + idempotency.

**Verified claims:**
- result_test.dart: fold(onSuccess, onFailure) routes Success to onSuccess, Failed to onFailure; map preserves failure object identity; flatMap chains Results with short-circuit on failure; Result.guard catches thrown Failures and wraps unknown errors in UnexpectedFailure(cause, isRetryable=false); toAsyncState converts Success→AsyncData, Failed→AsyncError
- usecase_test.dart: UseCase.call wraps thrown Failure objects as Failed (preserving the exception), and wraps non-Failure exceptions in Failed<UnexpectedFailure(cause:exception, isRetryable:false)>
- usecase_test.dart: StreamUseCase.call forwards Success/Failed results from execute stream; catches stream errors (e.g. StateError) and emits a trailing Failed<UnexpectedFailure(cause:error)> event, then terminates
- activity_test.dart: ActivityTracker.track uses ref-counted isLoading (increments on call, decrements on completion regardless of success/throw, but post-dispose decrements are suppressed); pending signal tracks active operation count
- activity_test.dart: RetryPolicy.delayFor(attempt) applies exponential backoff: delayFor(1) = delay, delayFor(2) = delay*backoffFactor, delayFor(3) = delay*backoffFactor², etc.; shouldRetry checks Failure.isRetryable by default, but custom retryIf predicate overrides
- controller_test.dart run: Success results do not emit to failures stream; run(emitFailure=true, default) emits one final failure after all retries exhausted; run(emitFailure=false) suppresses failure emission; retries only happen if Failure.isRetryable=true, up to maxAttempts total attempts (not retries); isLoading tracks via ActivityTracker (increments on run entry, decrements on run exit)
- controller_test.dart runInto: loads AsyncLoading; on success sets AsyncData; keeps stale data visible during reload (AsyncDataReloading state with hasValue=true, requireValue returns old value); on failure sets AsyncError
- controller_test.dart watch: StreamUseCase subscription routes Success events to onData callback, Failed events to failures broadcast stream, on dispose cancels subscription immediately (no further events)
- controller_test.dart dispose: runs onDispose callbacks in reverse order (LIFO), is idempotent (multiple calls invoke callbacks only once), sets isDisposed flag, cancels all watched stream subscriptions; post-dispose signal writes can be guarded with isDisposed check
- controller_test.dart defines helper: FlakyUseCase(failuresBeforeSuccess, failure) tracks attempts counter, fails first N times with given Failure, succeeds on attempt N+1 (or later); NetworkFailure has isRetryable=true
- controller_test.dart defines helper: SlowUseCase(gate:Completer) awaits gate.future, then returns Success(5); simulates long-running operation
- controller_test.dart defines helper: TickerUseCase (StreamUseCase) yields Success(1) through Success(params), then yields Failed(NetworkFailure('tick lost'))
- controller_test.dart defines helper: TestController exposes exec(useCase, params, retry, emitFailure), execInto(useCase, params, into), listen(useCase, params, onData), addCleanup(fn), disposed flag; wraps Controller methods for testing
- usecase_test.dart defines test helpers: Doubler(params:int)→Success(params*2), ThrowsFailure()→throws NetworkFailure, ThrowsError()→throws FormatException(non-Failure), CountingStream(params:int)→yields Success(1..params), FailingStream()→yields Success(1) then throws StateError
- result_test.dart: asyncStateSignal<T>() initializes to AsyncLoading<T> state (not data or error)
- app_test.dart: Integration tests verify retry absorbs first N failures (FakeTeamApi with failuresBeforeSuccess:2, RetryPolicy(maxAttempts:3) succeeds on third attempt); search filter uses computed signal; profile edits propagate via app-scoped shared signal; ValidationFailure surfaces through failures stream as snackbar

## cupline-orders-retrofit-surface

**Summary:** The orders page retrofit surface spans two crates with precise type boundaries. OrdersPage (cl-dashboard/src/pages/orders.rs) is a Leptos component managing a polling signal that calls list_orders and set_order_status from cl-ui/src/api/dashboard.rs, displaying OrderDto items grouped by status column. Signals flow through ShopState context (provided by DashboardShell in shell.rs), and API calls route through a gloo-net client with 401 silent-refresh behavior and ApiError enum. The routing structure nests OrdersPage as a route child under DashboardShell (ParentRoute "/").

**Caveats:** The polling loop uses gloo_timers::future::TimeoutFuture, which is a browser API only available in WASM. Orders page will not poll in non-WASM environments (test/SSR), causing reload signal to never increment beyond manual invocations. OrderDto fields are snapshot copies from the server; changes to items list or selected_options structure on the API side require coordinated updates here. The 401 refresh flow assumes refresh_token exists in localStorage; if the refresh endpoint fails, all subsequent requests will receive Unauthorized immediately until re-login.

**Verified claims:**
- OrdersPage component location and structure: /home/ed/Dev/personal/cupline/crates/cl-dashboard/src/pages/orders.rs lines 32-187. Component signals: orders (RwSignal<Option<Vec<OrderDto>>>), reload (RwSignal<u32>). Effects: Effect on state.selected + reload tracking calls list_orders(shop_id, false); 8s polling loop via gloo_timers::future::TimeoutFuture::new(8_000) increments reload. Action closure advance calls set_order_status(order_id, status) and updates reload. View groups orders by status field into 3 columns (new/preparing/ready) using OrderDto fields: ticket_number, placed_at, total, currency, items[].
- ShopState struct and context provision: /home/ed/Dev/personal/cupline/crates/cl-dashboard/src/pages/util.rs lines 32-79. ShopState fields: memberships RwSignal<Vec<MembershipDto>>, loaded RwSignal<bool>, selected RwSignal<Option<Uuid>>. provide_shop_state() creates and provides context. use_shop_state() retrieves it via expect_context().
- API function signatures in /home/ed/Dev/personal/cupline/crates/cl-ui/src/api/dashboard.rs lines 20-37. list_orders(shop_id: Uuid, all: bool) -> Result<Vec<OrderDto>, ApiError> (GET /api/v1/dashboard/shops/{shop_id}/orders). set_order_status(order_id: Uuid, status: &str) -> Result<OrderDto, ApiError> (PATCH /api/v1/dashboard/orders/{id} with UpdateOrderStatusRequest body).
- ApiError enum with 8 variants in /home/ed/Dev/personal/cupline/crates/cl-ui/src/api/error.rs lines 10-44: Network(String), Unauthorized, NotFound, Conflict, UnprocessableEntity(String), RateLimited, Server(String), Parse(String). Implements Serialize + Deserialize.
- Gloo-net client machinery in /home/ed/Dev/personal/cupline/crates/cl-ui/src/api/client.rs: authenticated_request (generic async fn) handles 401 with silent refresh. On 401: calls try_refresh() which POSTs refresh_token to /api/v1/auth/refresh, stores new tokens in localStorage, retries original request once. On refresh failure: force_logout() clears auth state, returns ApiError::Unauthorized. Public API: get<T>(path) -> Result<T, ApiError>, post<B,T>(path, body) -> Result<T, ApiError>, patch<B,T>(path, body) -> Result<T, ApiError>, delete<T>(path) -> Result<T, ApiError>.
- OrderDto struct in /home/ed/Dev/personal/cupline/crates/cl-core/src/dto.rs lines 180-202. Fields: id (Uuid), ticket_number (i32), status (String, values: 'new'|'preparing'|'ready'|'completed'|'cancelled'), total (i64, minor units), currency (String), placed_at (DateTime<Utc>), ready_at (Option<DateTime<Utc>>), items (Vec<OrderItemDto>). OrderItemDto has: name (String), price (i64), qty (i32), selected_options (serde_json::Value).
- UpdateOrderStatusRequest struct in /home/ed/Dev/personal/cupline/crates/cl-core/src/dto.rs lines 215-218. Single field: status (String).
- DashboardShell component in /home/ed/Dev/personal/cupline/crates/cl-dashboard/src/pages/shell.rs lines 29-172 provides context. Calls provide_shop_state() line 32 and provide_dash_locale() line 33. Loads shop memberships via Effect (lines 45-60) by calling cl_ui::api::auth_api::me() -> Result<MeResponse, ApiError> which returns {user, memberships}. Sets state.selected to first membership's shop_id. Renders shop switcher dropdown populated from state.memberships and tab navigation (lines 144-158) with routes: /=Orders, /menu, /options, /sales, /theme, /staff. Renders <Outlet/> for child routes (line 166).
- Routing configuration in /home/ed/Dev/personal/cupline/crates/cl-dashboard/src/lib.rs lines 36-56. DashboardApp component contains Router with Routes fallback. ParentRoute path='/' view=DashboardShell nests: Route path='' view=OrdersPage, Route path='menu' view=MenuPage, Route path='options' view=OptionsPage, Route path='sales' view=SalesPage, Route path='theme' view=ThemePage, Route path='staff' view=StaffPage. LoginPage mounted at /login (line 44).
- API base URL resolution and client initialization in /home/ed/Dev/personal/cupline/crates/cl-ui/src/api/client.rs lines 34-55. api_base() resolves from compile-time API_BASE_URL env var (with localhost→hostname substitution), falls back to browser origin with port 8080. All authenticated requests include 'Authorization: Bearer {token}' header (line 121). Token stored/retrieved from localStorage 'auth_token' key (lines 100-102). Refresh token stored in localStorage 'refresh_token' key (line 168).
- next_status() state machine in orders.rs lines 23-29 maps: 'new'→'preparing', 'preparing'→'ready', default→'completed'. Orders can also transition to 'cancelled' via cancel button (line 156). The page does not validate status transitions; server enforces legal transitions.
- OrdersPage polling is implemented with cfg(target_arch='wasm32') guards: main Effect at lines 39-51 runs on shop selection change, async Effect at lines 58-66 spawns a loop incrementing reload counter every 8 seconds. Polling stops on component unmount via on_cleanup (line 57) setting alive StoredValue to false. Non-WASM builds compile these out (lines 49-50, 75-76).

## wasm-portable-channels-timers

**Summary:** For wasm32-unknown-unknown (browser), **do not use tokio::sync::broadcast or tokio::time::sleep**—both panic due to Atomics/std::time::Instant incompatibilities. Best practice is callback-registry patterns (Leptos signals + callbacks) or async-broadcast for controlled environments. Use wasm-timer v0.2.5 or gloo-timers for cross-platform delays. Cfg-gating is the norm; tokio's "sync" feature is nominally stable on wasm32 but unreliable in practice.

**Caveats:** **Critical caveat:** tokio::sync and tokio::time on wasm32-unknown-unknown are only stable for wasm32-wasi (not wasm32-unknown-unknown). The search results document many panics and blocking issues. For wasm32-unknown-unknown (browser/JS runtime), **do not rely on any tokio features for critical path code**. If targeting wasm32-wasi (server-side WASM), tokio features are more reliable but still limited. Version tracking: Tokio 1.52.3 (May 2026), async-broadcast 0.7.2 (May 2026), event-listener 5.4.1, wasm-timer 0.2.5. No maintained futures-timer WASM alternative found; wasm-timer is the de facto standard."

**Verified claims:**
- tokio::sync::broadcast causes panics on wasm32-unknown-unknown due to Atomics.wait blocking the main thread
- tokio::time::sleep panics on wasm32-unknown-unknown (std::time::Instant::now is unavailable)
- async-broadcast v0.7.2 (May 2026) also problematic on wasm32-unknown-unknown due to event-listener dependency using Mutex/Atomics
- Leptos and Yew UI frameworks do not use broadcast channels; instead use signal-based reactivity (Leptos) or component re-render patterns (Yew)
- wasm-timer v0.2.5 is a maintained cross-platform sleep abstraction: uses tokio-timer on native targets, gloo/web-sys on wasm
- gloo-timers TimeoutFuture is designed specifically for wasm32-unknown-unknown and is actively used in Leptos ecosystem
- Tokio v1.52.3 (May 2026 latest) nominally supports sync feature on wasm32-unknown-unknown but is unreliable in practice
- cfg-gating with #[cfg(all(target_family = "wasm", target_os = "unknown"))] or #[cfg(not(target_arch = "wasm32"))] is standard practice
- tokio-with-wasm crate available as alternative providing tokio-like API for wasm32-unknown-unknown using JavaScript glue
- futures-timer has fundamental issues with wasm32-unknown-unknown due to Instant API limitations

