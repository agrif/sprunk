use rand::seq::IteratorRandom;
use rand::Rng;

const MAX_DRAWS: usize = 32;

pub struct RandomMixer<K> {
    max_history: usize,
    no_repeat_window: f32,
    history: std::collections::VecDeque<K>,
}

pub struct Possibility<'a, T, K> {
    mixer: &'a mut RandomMixer<K>,
    idx: Option<usize>,
    key: K,
    value: T,
}

impl<K> RandomMixer<K> {
    pub fn new(max_history: usize, no_repeat_window: f32) -> Self {
        Self {
            max_history,
            no_repeat_window,
            history: std::collections::VecDeque::with_capacity(max_history + 1),
        }
    }

    pub fn possibility<I, T, F>(&mut self, it: I, f: F) -> Option<Possibility<T, K>>
    where
        I: Iterator<Item = T> + Clone,
        F: Fn(&T) -> &K,
        K: std::cmp::Eq + Clone,
    {
        let rng = &mut rand::thread_rng();
        let recent = (self.no_repeat_window as f32 * self.history.len() as f32).round() as usize;
        let history_entries = self.history.len();

        let mut chosen = None;
        let mut idx = None;
        for _ in 0..MAX_DRAWS {
            chosen = it.clone().choose(rng);

            if let Some(choice) = &chosen {
                idx = self.history.iter().position(|x| x == f(choice));

                // succeed if not found in history
                let Some(idx_val) = idx else {
                    break;
                };

                // never select something in recent history
                if idx_val < recent {
                    continue;
                }

                // otherwise, roll under the index to succeed
                // (more recent items have lower index)
                if rng.gen_range(0..history_entries * history_entries) < idx_val * idx_val {
                    break;
                }
            } else {
                // empty iterator, no choice possible
                break;
            }
        }

        chosen.map(move |v| Possibility {
            mixer: self,
            idx,
            key: f(&v).clone(),
            value: v,
        })
    }

    pub fn choose<I, T, F>(&mut self, it: I, f: F) -> Option<T>
    where
        I: Iterator<Item = T> + Clone,
        F: Fn(&T) -> &K,
        K: std::cmp::Eq + Clone,
    {
        self.possibility(it, f).map(|p| p.accept())
    }
}

impl<'a, T, K> Possibility<'a, T, K> {
    pub fn map<F, U>(self, f: F) -> Possibility<'a, U, K>
    where
        F: FnOnce(T) -> U,
    {
        Possibility {
            mixer: self.mixer,
            idx: self.idx,
            key: self.key,
            value: f(self.value),
        }
    }

    pub fn accept(self) -> T {
        if let Some(idx) = self.idx {
            self.mixer.history.remove(idx);
        }
        self.mixer.history.push_front(self.key);
        self.mixer.history.truncate(self.mixer.max_history);

        self.value
    }
}
