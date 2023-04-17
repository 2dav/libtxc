#![allow(missing_docs)]

use std::fmt::Debug;

/// Аналог [`std::iter::Iterator`] для многопоточного использования.
///
/// Комбинаторы в этом трейте повторяют основные из [`std::iter::Iterator`], имеют такую
/// же семантику и гарантии *zero-cost abstractions*. Основные отличия заключаются в возможности
/// комбинаторов изменять своё состояние, требованиях [`Send`] + [`Sync`] и использовании обратного
/// вызова в качестве обработчика поступающих данных(аналог [`std::iter::Iterator::for_each`]).
///
/// В **libtxc** [Stream] используется для компоновки конвейера обработки входящих сообщений, который
/// запускается в потоке данных Transaq XML Connector, см. [`TransaqConnector::input_stream()`](crate::TransaqConnector::input_stream).
///
/// Cм. [examples](https://github.com/2dav/libtxc/examples) для примеров использования.
pub trait Stream: Sized + Send {
    type Output;

    fn subscribe<F: FnMut(Self::Output) + Sync + Send + 'static>(self, f: F);

    #[inline(always)]
    fn map<F, R>(self, f: F) -> Map<Self, F>
    where
        F: FnMut(Self::Output) -> R + Sync + Send,
    {
        Map { inner: self, f }
    }

    #[inline(always)]
    fn filter<F>(self, f: F) -> Filter<Self, F>
    where
        F: FnMut(&Self::Output) -> bool + Sync + Send,
    {
        Filter { inner: self, f }
    }

    #[inline(always)]
    fn filter_map<F, T>(self, f: F) -> FilterMap<Self, F>
    where
        F: FnMut(Self::Output) -> Option<T> + Sync + Send,
    {
        FilterMap { inner: self, f }
    }

    #[inline(always)]
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        F: FnMut(&Self::Output) + Sync + Send,
    {
        Inspect { inner: self, f }
    }
}

pub struct Map<S, F> {
    inner: S,
    f: F,
}
impl<S: Stream + Debug, F> Debug for Map<S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Map").field("inner", &self.inner).finish()
    }
}
impl<S, F, R> Stream for Map<S, F>
where
    S: Stream,
    F: FnMut(S::Output) -> R + Sync + Send + 'static,
{
    type Output = R;

    #[inline(always)]
    fn subscribe<FSub: FnMut(Self::Output) + Sync + Send + 'static>(self, mut f: FSub) {
        let mut mapf = self.f;
        self.inner.subscribe(move |x| f((mapf)(x)));
    }
}

pub struct Filter<S, F> {
    inner: S,
    f: F,
}
impl<S: Stream + Debug, F> Debug for Filter<S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Filter").field("inner", &self.inner).finish()
    }
}
impl<S, F> Stream for Filter<S, F>
where
    S: Stream,
    F: FnMut(&S::Output) -> bool + Sync + Send + 'static,
{
    type Output = S::Output;

    #[inline(always)]
    fn subscribe<FSub: FnMut(Self::Output) + Sync + Send + 'static>(self, mut f: FSub) {
        let mut filterf = self.f;
        self.inner.subscribe(move |x| {
            if (filterf)(&x) {
                f(x)
            }
        });
    }
}

pub struct FilterMap<S, F> {
    inner: S,
    f: F,
}
impl<S: Stream + Debug, F> Debug for FilterMap<S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterMap").field("inner", &self.inner).finish()
    }
}
impl<S, F, T> Stream for FilterMap<S, F>
where
    S: Stream,
    F: FnMut(S::Output) -> Option<T> + Sync + Send + 'static,
{
    type Output = T;

    #[inline(always)]
    fn subscribe<FSub: FnMut(Self::Output) + Sync + Send + 'static>(self, mut f: FSub) {
        let mut fmapf = self.f;
        self.inner.subscribe(move |x| {
            if let Some(x) = (fmapf)(x) {
                f(x);
            }
        })
    }
}

pub struct Inspect<S, F> {
    inner: S,
    f: F,
}
impl<S: Stream + Debug, F> Debug for Inspect<S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inspect").field("inner", &self.inner).finish()
    }
}
impl<S, F> Stream for Inspect<S, F>
where
    S: Stream,
    F: FnMut(&S::Output) + Sync + Send + 'static,
{
    type Output = S::Output;

    #[inline(always)]
    fn subscribe<FSub: FnMut(Self::Output) + Sync + Send + 'static>(self, mut f: FSub) {
        let mut inspectf = self.f;
        self.inner.subscribe(move |x| {
            (inspectf)(&x);
            f(x)
        })
    }
}
