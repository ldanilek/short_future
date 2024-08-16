use std::marker::PhantomData;

use futures::{future::BoxFuture, Future};

/// ShortBoxFuture<'b, 'a, T> is a future with a shorter lifetime than both 'a and 'b.
/// It is equivalent to BoxFuture<'a + 'b, T> or
/// BoxFuture<'b, T> where 'a > 'b.
pub struct ShortBoxFuture<'b, 'a: 'b, T>(pub BoxFuture<'b, T>, PhantomData<&'a ()>);
impl<'b, 'a: 'b, T, F: Future<Output = T> + Send + 'b> From<F> for ShortBoxFuture<'b, 'a, T> {
    fn from(f: F) -> Self {
        Self(Box::pin(f), PhantomData)
    }
}

#[cfg(test)]
mod tests {
    use super::ShortBoxFuture;

    pub async fn with_retries<'a, F>(f: F) -> usize
    where
        F: for<'b> Fn(&'b str) -> ShortBoxFuture<'b, 'a, Result<(), ()>>,
    {
        for i in 0..100 {
            // Imagine the transaction cannot be cloned or moved, because it
            // represents a persistent database connection.
            let transaction = format!("{i} transaction");
            let result = f(&transaction).0.await;
            // Imagine this is a commit / rollback.
            drop(transaction);
            match result {
                Ok(()) => return i,
                Err(()) => {}
            }
        }
        0
    }

    pub async fn str_eq<'a, 'b>(a: &'a str, b: &'b str) -> Result<(), ()> {
        if a == b {
            Ok(())
        } else {
            Err(())
        }
    }

    #[tokio::test]
    async fn test_retries_closure() {
        // Imagine this data is large and expensive to clone.
        let data = format!("11 transaction");
        let result = with_retries(|session| async { str_eq(session, &data).await }.into()).await;
        assert_eq!(result, 11);
    }

    #[tokio::test]
    async fn test_retries_fn() {
        let data = format!("11 transaction");
        let result = with_retries(|session| str_eq(session, &data).into()).await;
        assert_eq!(result, 11);
    }

    /// You can also do logic in the closure, as long as the arguments are
    /// wrapped. Compare to `test_retries_inline` below, which doesn't work.
    #[tokio::test]
    async fn test_retries_semi_inline() {
        struct WrapStr<'a>(&'a str);
        let data = format!("11 transaction");
        let result = with_retries(|session| {
            async {
                if WrapStr(session).0 == &data {
                    Ok(())
                } else {
                    Err(())
                }
            }
            .into()
        })
        .await;
        assert_eq!(result, 11);
    }
}

/// You may be thinking "the borrow checker is smart and I'm clever.
/// I can do this with lifetimes/HRTBs/other".
/// Prepare to be disappointed. These are examples of what doesn't work.
/// Remove the #[cfg(any())] directives to see the errors.
#[cfg(test)]
#[allow(unused_imports)]
#[allow(dead_code)]
mod failing_tests {
    use super::{
        tests::{str_eq, with_retries},
        ShortBoxFuture,
    };
    use futures::{
        future::{BoxFuture, Future},
        FutureExt,
    };
    use std::{marker::PhantomData, pin::Pin};

    /// Error: "type alias takes one generic but two generic arguments were supplied"
    #[cfg(any())]
    async fn with_retries_boxfuture_multi_bound<'a, F>(f: F)
    where
        F: for<'b> Fn(&'b str) -> BoxFuture<'a + 'b, ()>,
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            let result = f(&transaction).await;
            drop(transaction);
        }
    }

    /// Error: "only a single explicit lifetime bound is permitted"
    #[cfg(any())]
    async fn with_retries_multi_bound<'a, F>(f: F)
    where
        F: for<'b> Fn(&'b str) -> Pin<Box<dyn Future<Output = ()> + 'a + 'b>>,
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            f(&transaction).await;
            drop(transaction);
        }
    }

    /// Error: "`impl Trait` is not allowed in the return type of `Fn` trait bounds"
    #[cfg(any())]
    async fn with_retries_impl_future<'a, F>(f: F)
    where
        F: for<'b> Fn(&'b str) -> (impl Future<Output = ()> + 'a + 'b),
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            f(&transaction).await;
            drop(transaction);
        }
    }

    /// This is fine on its own, but the closure can't borrow from the
    /// enclosing scope (see test_single_hrtb).
    async fn with_retries_single_hrtb<F>(f: F)
    where
        F: for<'a> Fn(&'a str) -> BoxFuture<'a, ()>,
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            f(&transaction).await;
            drop(transaction);
        }
    }

    /// Error: "`data` does not live long enough"
    #[cfg(any())]
    async fn test_single_hrtb() {
        let data = format!("11 transaction");
        with_retries_single_hrtb(|session| {
            async {
                str_eq(session, &data);
            }
            .boxed()
        })
        .await;
    }

    /// Error: "argument requires that `transaction` is borrowed for `'a`"
    #[cfg(any())]
    async fn with_retries_no_hrtb<'a, F>(f: F)
    where
        F: Fn(&'a str) -> BoxFuture<'a, ()>,
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            f(&transaction).await;
            drop(transaction);
        }
    }

    /// Error: "bounds cannot be used in this context"
    #[cfg(any())]
    pub async fn with_retries_hrtb_dependency<'a, F>(f: F)
    where
        F: for<'b: 'a> Fn(&'b str) -> BoxFuture<'b, ()>,
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            f(&transaction).await;
            drop(transaction);
        }
    }

    /// The worst news: even ShortBoxFuture isn't perfect.
    /// This is the same as super::tests::test_retries_closure but
    /// `str_eq` is inlined. It's the same as
    /// super::tests::test_retries_semi_inline minus the WrapStr.
    ///
    /// Error: "async block may outlive the current function, but it borrows
    /// `session`, which is owned by the current function"
    #[cfg(any())]
    #[tokio::test]
    async fn test_retries_fully_inline() {
        let data = format!("11 transaction");
        let result = with_retries(|session| {
            async {
                if session == &data {
                    Ok(())
                } else {
                    Err(())
                }
            }
            .into()
        })
        .await;
        assert_eq!(result, 11);
    }

    /// More bad news: ShortBoxFuture's lifetime bounds are not symmetric.
    /// The first lifetime bound should come from the HRTB.
    /// Error: "`transaction` does not live long enough"
    #[cfg(any())]
    async fn with_retries_flipped_bound_order<'a, F>(f: F)
    where
        F: for<'b> Fn(&'b str) -> ShortBoxFuture<'a, 'b, ()>,
    {
        for i in 0..100 {
            let transaction = format!("{i} transaction");
            f(&transaction).0.await;
            drop(transaction);
        }
    }
}
