//! [`SigningStepManager`] — hooks fired around the signature steps of
//! a cross-chain order post.
//!
//! Mirrors the `SigningStepManager` interface of the `TypeScript`
//! [`@cowprotocol/sdk-bridging`](https://github.com/cowprotocol/cow-sdk)
//! package. Callers attach callbacks to observe (or abort) the flow
//! between the bridge-hook signature and the order signature:
//!
//! ```text
//! before_bridging_sign? ─► sign bridge hook ─► after_bridging_sign?
//!                                  │
//!                                  │ (on error → on_bridging_sign_error)
//!                                  ▼
//!                           before_order_sign? ─► sign + post order ─► after_order_sign?
//!                                                       │
//!                                                       │ (on error → on_order_sign_error)
//!                                                       ▼
//!                                                  propagate error
//! ```
//!
//! All six fields are optional. The `before_*` / `after_*` callbacks are
//! async and their errors propagate (so a UI can abort the flow by
//! returning `Err`); the `on_*_error` callbacks are synchronous and
//! purely observational.
//!
//! # Send-ness
//!
//! On native targets the callbacks are `Send + Sync` so a single
//! [`SigningStepManager`] can be shared across tasks. On WASM the
//! bounds are relaxed through [`MaybeSendSync`](crate::provider::MaybeSendSync).

use std::pin::Pin;

use cow_errors::CowError;

/// Future returned by an async step callback. The future owns all its
/// state (`'static`) so callers can keep the [`SigningStepManager`] for
/// the full duration of a post and re-fire callbacks without caring
/// about borrowed captures.
#[cfg(not(target_arch = "wasm32"))]
pub type StepFuture =
    Pin<Box<dyn std::future::Future<Output = Result<(), CowError>> + Send + 'static>>;

/// Future returned by an async step callback (WASM variant — `!Send`).
#[cfg(target_arch = "wasm32")]
pub type StepFuture = Pin<Box<dyn std::future::Future<Output = Result<(), CowError>> + 'static>>;

/// Async callback type for the `before_*` / `after_*` hooks.
#[cfg(not(target_arch = "wasm32"))]
pub type StepFn = Box<dyn Fn() -> StepFuture + Send + Sync>;

/// Async callback type for the `before_*` / `after_*` hooks (WASM).
#[cfg(target_arch = "wasm32")]
pub type StepFn = Box<dyn Fn() -> StepFuture>;

/// Synchronous callback type for the `on_*_error` hooks.
#[cfg(not(target_arch = "wasm32"))]
pub type ErrFn = Box<dyn Fn(&CowError) + Send + Sync>;

/// Synchronous callback type for the `on_*_error` hooks (WASM).
#[cfg(target_arch = "wasm32")]
pub type ErrFn = Box<dyn Fn(&CowError)>;

/// Six-slot callback bundle fired around the signature steps of a
/// cross-chain order post.
///
/// Build one with [`SigningStepManager::new`] and attach callbacks via
/// the `with_*` builders. All slots are optional; the default instance
/// is a no-op that behaves exactly as if no manager were passed.
#[derive(Default)]
pub struct SigningStepManager {
    /// Fires before the bridge hook is signed. Return `Err` to abort.
    pub before_bridging_sign: Option<StepFn>,
    /// Fires after the bridge hook is signed successfully.
    pub after_bridging_sign: Option<StepFn>,
    /// Fires before the `CoW` order is signed / posted. Return `Err` to abort.
    pub before_order_sign: Option<StepFn>,
    /// Fires after the `CoW` order is posted successfully.
    pub after_order_sign: Option<StepFn>,
    /// Fires synchronously when bridge-hook signing fails.
    pub on_bridging_sign_error: Option<ErrFn>,
    /// Fires synchronously when order signing / posting fails.
    pub on_order_sign_error: Option<ErrFn>,
}

impl std::fmt::Debug for SigningStepManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningStepManager")
            .field("before_bridging_sign", &self.before_bridging_sign.is_some())
            .field("after_bridging_sign", &self.after_bridging_sign.is_some())
            .field("before_order_sign", &self.before_order_sign.is_some())
            .field("after_order_sign", &self.after_order_sign.is_some())
            .field("on_bridging_sign_error", &self.on_bridging_sign_error.is_some())
            .field("on_order_sign_error", &self.on_order_sign_error.is_some())
            .finish()
    }
}

impl SigningStepManager {
    /// Construct an empty [`SigningStepManager`] with no callbacks.
    ///
    /// Equivalent to [`SigningStepManager::default`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a `before_bridging_sign` callback.
    ///
    /// Fires immediately before the bridge hook is signed; returning an
    /// `Err` aborts the post flow.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_before_bridging_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + Send + Sync + 'static,
    {
        self.before_bridging_sign = Some(Box::new(f));
        self
    }

    /// Attach a `before_bridging_sign` callback (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_before_bridging_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + 'static,
    {
        self.before_bridging_sign = Some(Box::new(f));
        self
    }

    /// Attach an `after_bridging_sign` callback.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_after_bridging_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + Send + Sync + 'static,
    {
        self.after_bridging_sign = Some(Box::new(f));
        self
    }

    /// Attach an `after_bridging_sign` callback (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_after_bridging_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + 'static,
    {
        self.after_bridging_sign = Some(Box::new(f));
        self
    }

    /// Attach a `before_order_sign` callback. Returning `Err` aborts the post.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_before_order_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + Send + Sync + 'static,
    {
        self.before_order_sign = Some(Box::new(f));
        self
    }

    /// Attach a `before_order_sign` callback (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_before_order_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + 'static,
    {
        self.before_order_sign = Some(Box::new(f));
        self
    }

    /// Attach an `after_order_sign` callback. Awaited on success.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_after_order_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + Send + Sync + 'static,
    {
        self.after_order_sign = Some(Box::new(f));
        self
    }

    /// Attach an `after_order_sign` callback (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_after_order_sign<F>(mut self, f: F) -> Self
    where
        F: Fn() -> StepFuture + 'static,
    {
        self.after_order_sign = Some(Box::new(f));
        self
    }

    /// Attach an `on_bridging_sign_error` callback — fires synchronously
    /// when bridge-hook signing fails.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_on_bridging_sign_error<F>(mut self, f: F) -> Self
    where
        F: Fn(&CowError) + Send + Sync + 'static,
    {
        self.on_bridging_sign_error = Some(Box::new(f));
        self
    }

    /// Attach an `on_bridging_sign_error` callback (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_on_bridging_sign_error<F>(mut self, f: F) -> Self
    where
        F: Fn(&CowError) + 'static,
    {
        self.on_bridging_sign_error = Some(Box::new(f));
        self
    }

    /// Attach an `on_order_sign_error` callback — fires synchronously when
    /// order signing / posting fails.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_on_order_sign_error<F>(mut self, f: F) -> Self
    where
        F: Fn(&CowError) + Send + Sync + 'static,
    {
        self.on_order_sign_error = Some(Box::new(f));
        self
    }

    /// Attach an `on_order_sign_error` callback (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_on_order_sign_error<F>(mut self, f: F) -> Self
    where
        F: Fn(&CowError) + 'static,
    {
        self.on_order_sign_error = Some(Box::new(f));
        self
    }

    // ── Sync-wrapping convenience helpers ─────────────────────────────
    //
    // The `with_before_*_sign` / `with_after_*_sign` builders above take
    // async callbacks, which is overkill for the common case (log a
    // line, update a progress bar, push to a channel). The `_sync`
    // variants accept a synchronous `FnOnce`-style closure and wrap it
    // in a resolved `async move { f(); Ok(()) }` future internally.
    //
    // Prefer the sync variants unless you genuinely need to await
    // something inside the callback.

    /// Synchronous-sugar for [`Self::with_before_bridging_sign`].
    ///
    /// The closure runs to completion on every fire; its return value is
    /// discarded. Use the async variant if you need to abort the post
    /// flow from the callback.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_before_bridging_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.with_before_bridging_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_before_bridging_sign`] (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_before_bridging_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.with_before_bridging_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_after_bridging_sign`].
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_after_bridging_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.with_after_bridging_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_after_bridging_sign`] (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_after_bridging_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.with_after_bridging_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_before_order_sign`].
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_before_order_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.with_before_order_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_before_order_sign`] (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_before_order_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.with_before_order_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_after_order_sign`].
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_after_order_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.with_after_order_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Synchronous-sugar for [`Self::with_after_order_sign`] (WASM).
    #[must_use]
    #[cfg(target_arch = "wasm32")]
    pub fn with_after_order_sign_sync<F>(self, f: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.with_after_order_sign(move || {
            f();
            Box::pin(async { Ok(()) })
        })
    }

    /// Invoke `before_bridging_sign` if present, propagating any error.
    ///
    /// # Errors
    ///
    /// Returns whatever the attached callback returns.
    pub async fn fire_before_bridging_sign(&self) -> Result<(), CowError> {
        if let Some(f) = &self.before_bridging_sign {
            f().await?;
        }
        Ok(())
    }

    /// Invoke `after_bridging_sign` if present.
    ///
    /// # Errors
    ///
    /// Returns whatever the attached callback returns.
    pub async fn fire_after_bridging_sign(&self) -> Result<(), CowError> {
        if let Some(f) = &self.after_bridging_sign {
            f().await?;
        }
        Ok(())
    }

    /// Invoke `before_order_sign` if present.
    ///
    /// # Errors
    ///
    /// Returns whatever the attached callback returns.
    pub async fn fire_before_order_sign(&self) -> Result<(), CowError> {
        if let Some(f) = &self.before_order_sign {
            f().await?;
        }
        Ok(())
    }

    /// Invoke `after_order_sign` if present.
    ///
    /// # Errors
    ///
    /// Returns whatever the attached callback returns.
    pub async fn fire_after_order_sign(&self) -> Result<(), CowError> {
        if let Some(f) = &self.after_order_sign {
            f().await?;
        }
        Ok(())
    }

    /// Invoke `on_bridging_sign_error` if present (synchronous).
    pub fn fire_on_bridging_sign_error(&self, err: &CowError) {
        if let Some(f) = &self.on_bridging_sign_error {
            f(err);
        }
    }

    /// Invoke `on_order_sign_error` if present (synchronous).
    pub fn fire_on_order_sign_error(&self, err: &CowError) {
        if let Some(f) = &self.on_order_sign_error {
            f(err);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    };

    #[allow(
        clippy::disallowed_types,
        reason = "test-only recorder — contention not a concern"
    )]
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn default_has_no_callbacks() {
        let mgr = SigningStepManager::default();
        assert!(mgr.before_bridging_sign.is_none());
        assert!(mgr.after_bridging_sign.is_none());
        assert!(mgr.before_order_sign.is_none());
        assert!(mgr.after_order_sign.is_none());
        assert!(mgr.on_bridging_sign_error.is_none());
        assert!(mgr.on_order_sign_error.is_none());
    }

    #[test]
    fn new_matches_default() {
        let a = SigningStepManager::new();
        let b = SigningStepManager::default();
        assert_eq!(
            format!("{a:?}").contains("SigningStepManager"),
            format!("{b:?}").contains("SigningStepManager"),
        );
    }

    #[test]
    fn debug_impl_reports_presence_booleans() {
        let mgr =
            SigningStepManager::new().with_before_bridging_sign(|| Box::pin(async { Ok(()) }));
        let dbg = format!("{mgr:?}");
        assert!(dbg.contains("before_bridging_sign: true"));
        assert!(dbg.contains("after_bridging_sign: false"));
    }

    #[tokio::test]
    async fn callbacks_fire_in_registered_order() {
        #[allow(clippy::disallowed_types, reason = "test-only recorder")]
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let o1 = Arc::clone(&order);
        let o2 = Arc::clone(&order);
        let o3 = Arc::clone(&order);
        let o4 = Arc::clone(&order);

        let mgr = SigningStepManager::new()
            .with_before_bridging_sign(move || {
                let o = Arc::clone(&o1);
                Box::pin(async move {
                    o.lock().unwrap().push("before_bridging_sign");
                    Ok(())
                })
            })
            .with_after_bridging_sign(move || {
                let o = Arc::clone(&o2);
                Box::pin(async move {
                    o.lock().unwrap().push("after_bridging_sign");
                    Ok(())
                })
            })
            .with_before_order_sign(move || {
                let o = Arc::clone(&o3);
                Box::pin(async move {
                    o.lock().unwrap().push("before_order_sign");
                    Ok(())
                })
            })
            .with_after_order_sign(move || {
                let o = Arc::clone(&o4);
                Box::pin(async move {
                    o.lock().unwrap().push("after_order_sign");
                    Ok(())
                })
            });

        mgr.fire_before_bridging_sign().await.unwrap();
        mgr.fire_after_bridging_sign().await.unwrap();
        mgr.fire_before_order_sign().await.unwrap();
        mgr.fire_after_order_sign().await.unwrap();

        let recorded: Vec<&str> = order.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec![
                "before_bridging_sign",
                "after_bridging_sign",
                "before_order_sign",
                "after_order_sign",
            ],
        );
    }

    #[tokio::test]
    async fn empty_manager_is_no_op() {
        let mgr = SigningStepManager::new();
        mgr.fire_before_bridging_sign().await.unwrap();
        mgr.fire_after_bridging_sign().await.unwrap();
        mgr.fire_before_order_sign().await.unwrap();
        mgr.fire_after_order_sign().await.unwrap();
        mgr.fire_on_bridging_sign_error(&CowError::AppData("x".into()));
        mgr.fire_on_order_sign_error(&CowError::AppData("x".into()));
    }

    #[tokio::test]
    async fn before_callback_error_is_propagated() {
        let mgr = SigningStepManager::new().with_before_bridging_sign(|| {
            Box::pin(async { Err(CowError::AppData("abort".into())) })
        });
        let err = mgr.fire_before_bridging_sign().await.unwrap_err();
        assert!(err.to_string().contains("abort"));
    }

    #[tokio::test]
    async fn before_order_sign_error_is_propagated() {
        let mgr = SigningStepManager::new()
            .with_before_order_sign(|| Box::pin(async { Err(CowError::AppData("stop".into())) }));
        let err = mgr.fire_before_order_sign().await.unwrap_err();
        assert!(err.to_string().contains("stop"));
    }

    #[test]
    fn sync_error_callbacks_fire_exactly_once() {
        let bridge_count = Arc::new(AtomicU8::new(0));
        let order_count = Arc::new(AtomicU8::new(0));
        let bc = Arc::clone(&bridge_count);
        let oc = Arc::clone(&order_count);

        let mgr = SigningStepManager::new()
            .with_on_bridging_sign_error(move |_err| {
                bc.fetch_add(1, Ordering::SeqCst);
            })
            .with_on_order_sign_error(move |_err| {
                oc.fetch_add(1, Ordering::SeqCst);
            });

        mgr.fire_on_bridging_sign_error(&CowError::AppData("e".into()));
        mgr.fire_on_order_sign_error(&CowError::AppData("e".into()));

        assert_eq!(bridge_count.load(Ordering::SeqCst), 1);
        assert_eq!(order_count.load(Ordering::SeqCst), 1);
    }

    // ── Sync-wrapping helpers ────────────────────────────────────────────

    #[tokio::test]
    async fn sync_helpers_fire_in_registered_order() {
        #[allow(clippy::disallowed_types, reason = "test-only recorder")]
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let o1 = Arc::clone(&order);
        let o2 = Arc::clone(&order);
        let o3 = Arc::clone(&order);
        let o4 = Arc::clone(&order);

        let mgr = SigningStepManager::new()
            .with_before_bridging_sign_sync(move || {
                o1.lock().unwrap().push("before_bridging_sign");
            })
            .with_after_bridging_sign_sync(move || {
                o2.lock().unwrap().push("after_bridging_sign");
            })
            .with_before_order_sign_sync(move || {
                o3.lock().unwrap().push("before_order_sign");
            })
            .with_after_order_sign_sync(move || {
                o4.lock().unwrap().push("after_order_sign");
            });

        mgr.fire_before_bridging_sign().await.unwrap();
        mgr.fire_after_bridging_sign().await.unwrap();
        mgr.fire_before_order_sign().await.unwrap();
        mgr.fire_after_order_sign().await.unwrap();

        let recorded: Vec<&str> = order.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec![
                "before_bridging_sign",
                "after_bridging_sign",
                "before_order_sign",
                "after_order_sign",
            ],
        );
    }

    #[tokio::test]
    async fn sync_helpers_always_return_ok() {
        let mgr = SigningStepManager::new()
            .with_before_bridging_sign_sync(|| {})
            .with_after_bridging_sign_sync(|| {})
            .with_before_order_sign_sync(|| {})
            .with_after_order_sign_sync(|| {});

        // Sync variant has no way to abort; each fire must succeed.
        assert!(mgr.fire_before_bridging_sign().await.is_ok());
        assert!(mgr.fire_after_bridging_sign().await.is_ok());
        assert!(mgr.fire_before_order_sign().await.is_ok());
        assert!(mgr.fire_after_order_sign().await.is_ok());
    }

    #[tokio::test]
    async fn sync_and_async_helpers_compose() {
        // Build a manager that mixes sync + async callbacks and check
        // they fire in the right order, with the async one's error
        // still propagating.
        let sync_fired = Arc::new(AtomicU8::new(0));
        let sf = Arc::clone(&sync_fired);
        let mgr = SigningStepManager::new()
            .with_before_bridging_sign_sync(move || {
                sf.fetch_add(1, Ordering::SeqCst);
            })
            .with_after_bridging_sign(|| {
                Box::pin(async move { Err(CowError::AppData("stop after bridging".into())) })
            });

        mgr.fire_before_bridging_sign().await.unwrap();
        assert_eq!(sync_fired.load(Ordering::SeqCst), 1);

        let err = mgr.fire_after_bridging_sign().await.unwrap_err();
        assert!(err.to_string().contains("stop after bridging"));
    }
}
