//! Rows that load a chunk at a time as the user scrolls: the first chunk
//! when what the rows answer changes (a search, a sort), the next when the
//! list comes near its last row. A refresh of the same query (an edit
//! elsewhere) loads as many rows as are shown, so the list stays put.
use crate::app::{App, run_service};
use nsh_core::prelude::*;
use std::{future::Future, pin::Pin, rc::Rc};

type Chunk<T> = Pin<Box<dyn Future<Output = Result<(Vec<T>, i64), String>> + Send>>;
/// Loads `limit` rows from `offset`: the rows and how many there are.
pub type Fetch<T> = Rc<dyn Fn(i64, i64) -> Chunk<T>>;

/// A fetcher from a closure returning the chunk's future.
pub fn fetcher<T, F>(f: impl Fn(i64, i64) -> F + 'static) -> Fetch<T>
where
    F: Future<Output = Result<(Vec<T>, i64), String>> + Send + 'static,
{
    Rc::new(move |offset, limit| Box::pin(f(offset, limit)) as Chunk<T>)
}

pub struct Paged<T: 'static> {
    /// The rows loaded so far, in order.
    pub rows: Signal<Rc<Vec<Rc<T>>>>,
    /// How many rows there are; `None` until the first chunk arrives.
    pub total: Signal<Option<i64>>,
    /// A chunk is on its way.
    pub loading: Signal<bool>,
    chunk: i64,
    /// Bumped by each new query: the chunks of an older one are dropped.
    generation: Stored<u64>,
    fetch: Stored<Option<Fetch<T>>>,
    app: App,
    owner: Owner,
}
impl<T> Clone for Paged<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Paged<T> {}

impl<T: Send + 'static> Paged<T> {
    /// Rows `source` fetches, `chunk` at a time. `source` runs again when
    /// a signal it reads changes and gives a key for what the rows answer
    /// with its fetcher: a new key starts over, the same key reloads what
    /// is shown. The first load also covers the `first` rows (a list
    /// coming back to where it was).
    pub fn new<K: PartialEq + 'static>(
        app: App,
        chunk: i64,
        first: i64,
        source: impl Fn() -> (K, Fetch<T>) + 'static,
    ) -> Self {
        let this = Self {
            rows: signal_with(Rc::new(Vec::new()), SignalEquals::Never),
            total: signal(None),
            loading: signal(false),
            chunk,
            generation: stored(0),
            fetch: stored(None),
            app,
            owner: Owner::current(),
        };
        let last = stored(None::<K>);
        let first = stored(Some(first));
        effect(move || {
            let (key, fetch) = source();
            untracked(|| {
                let same = last.with(|k| k.as_ref() == Some(&key));
                last.with(|k| *k = Some(key));
                let shown = if same {
                    let shown = this.rows.with(|r| r.len() as i64);
                    // Every row was here: rows added since come too.
                    let all = this.total.get().is_some_and(|t| shown >= t);
                    if all { shown + chunk } else { shown }
                } else {
                    this.rows.set(Rc::new(Vec::new()));
                    this.total.set(None);
                    first.with(Option::take).map_or(0, |n| n + chunk)
                };
                this.fetch.with(|f| *f = Some(fetch));
                this.generation.with(|g| *g += 1);
                this.load(0, shown.max(chunk), true);
            });
        });
        this
    }

    fn load(self, offset: i64, limit: i64, replace: bool) {
        let Some(chunk) = self.fetch.with(|f| f.as_ref().map(|f| f(offset, limit))) else {
            return;
        };
        let generation = self.generation.with(|g| *g);
        self.loading.set(true);
        let task = run_service(chunk);
        self.owner.run(|| {
            spawn(async move {
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r);
                if !self.owner.is_live() || self.generation.with(|g| *g) != generation {
                    return;
                }
                self.loading.set(false);
                match result {
                    Ok((items, total)) => {
                        self.total.set(Some(total));
                        let fresh = items.into_iter().map(Rc::new);
                        if replace {
                            self.rows.set(Rc::new(fresh.collect()));
                        } else {
                            self.rows.update(|rows| {
                                let mut next = Vec::with_capacity(rows.len() + limit as usize);
                                next.extend(rows.iter().cloned());
                                next.extend(fresh);
                                *rows = Rc::new(next);
                            });
                        }
                    }
                    Err(e) => self.app.message.set(e),
                }
            })
        });
    }

    /// Loads the next chunk, unless one is on its way or all are here.
    pub fn more(self) {
        if self.loading.get_untracked() {
            return;
        }
        let shown = self.rows.with_untracked(|r| r.len() as i64);
        if self.total.get_untracked().is_some_and(|t| shown < t) {
            self.load(shown, self.chunk, false);
        }
    }

    /// How many rows are loaded.
    pub fn len(self) -> usize {
        self.rows.with(|r| r.len())
    }

    /// Whether rows remain to load.
    pub fn has_more(self) -> bool {
        let shown = self.len() as i64;
        self.total.get().is_some_and(|t| shown < t)
    }

    /// The row at `i`, if loaded.
    pub fn at(self, i: usize) -> Option<Rc<T>> {
        self.rows.with(|r| r.get(i).cloned())
    }

    /// "120 of 2,419 tags" while more remain, else "2,419 tags".
    pub fn caption(self, one: &str, many: &str) -> String {
        match self.total.get() {
            None | Some(0) => String::new(),
            Some(total) => {
                let shown = self.len() as i64;
                let all = crate::ui::counted(total, one, many);
                if shown < total {
                    format!("{shown} of {all}")
                } else {
                    all
                }
            }
        }
    }
}
