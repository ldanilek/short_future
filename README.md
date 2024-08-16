# Futures with short lifetimes

short_future is a library defining a utility for working with Future lifetimes:
`ShortBoxFuture<'a, 'b, T>` can capture borrows with lifetimes of `'a` and `'b`
at the same time.

`ShortBoxFuture` works around limitations of HRTBs and explicit lifetime
bounds. This is useful when wrapping async closures, where the closure
returns a future that depends on both:

1. references in the enclosing scope with lifetime 'a.
2. references in the closure's arguments with lifetime 'b.

For example, you can write a helper that retries a database operation, where
a new transaction is created for every retry, and the data is borrowed from
the enclosing scope:

```rust
async fn run_twice<'a>(&'a self, f: F) -> anyhow::Result<()>
where F: for<'b> Fn(&'b mut Transaction) -> ShortBoxFuture<'b, 'a, anyhow::Result<()>>
{
    for i in 0..2 {
        let mut tx = self.begin();
        f(&mut tx).0.await?;
    }
    Ok(())
}

async fn go(&self) {
    let data = get_data();
    self.run_twice(|tx| async {
        tx.get(&data.id).await;
    }.into()).await
}
```

See the tests for more examples, and for demonstrations of the issues that
necessitate `ShortBoxFuture`.