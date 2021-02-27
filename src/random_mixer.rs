use rand::seq::IteratorRandom;

pub struct RandomMixer<K> {
    _tmp: std::collections::HashMap<K, u8>,
}

pub struct Possibility<'a, T, K> {
    mixer: &'a mut RandomMixer<K>,
    value: T,
}

impl<K> RandomMixer<K> {
    pub fn new() -> Self {
        Self {
            _tmp: std::collections::HashMap::new(),
        }
    }

    pub fn possibility<I, T>(&mut self, it: I) -> Option<Possibility<T, K>>
    where
        I: Iterator<Item = T>,
    {
        it.choose(&mut rand::thread_rng())
            .map(move |v| Possibility {
                mixer: self,
                value: v,
            })
    }

    pub fn choose<I, T, F>(&mut self, it: I, f: F) -> Option<T>
    where
        I: Iterator<Item = T>,
        F: Fn(&T) -> &K,
        K: std::cmp::Eq + std::hash::Hash,
    {
        self.possibility(it).map(|p| p.accept(f))
    }
}

impl<'a, T, K> Possibility<'a, T, K> {
    pub fn map<F, U>(self, f: F) -> Possibility<'a, U, K>
    where
        F: FnOnce(T) -> U,
    {
        Possibility {
            mixer: self.mixer,
            value: f(self.value),
        }
    }

    pub fn accept<F>(self, _f: F) -> T
    where
        F: Fn(&T) -> &K,
        K: std::cmp::Eq + std::hash::Hash,
    {
        self.value
    }
}
