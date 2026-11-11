use std::collections::HashSet;

use rand::Rng;

use crate::model::PlayMode;

pub const HISTORY_CAP: usize = 100;

#[derive(Debug, PartialEq, Eq)]
pub enum QueueError {
    Empty,
    AtEnd,
}

pub struct Queue {
    ids: Vec<String>,
    current: Option<usize>,
    mode: PlayMode,
    played: HashSet<usize>,
    history: Vec<String>,
}

impl Queue {
    pub fn new(mode: PlayMode) -> Self {
        Self {
            ids: Vec::new(),
            current: None,
            mode,
            played: HashSet::new(),
            history: Vec::new(),
        }
    }

    pub fn set_mode(&mut self, mode: PlayMode) {
        if self.mode != mode {
            self.played.clear();
        }
        self.mode = mode;
    }

    /// 替换范围快照；current_id 在新列表里则保留位置，否则 current=None。
    pub fn set_ids(&mut self, ids: Vec<String>, current_id: Option<&str>) {
        self.ids = ids;
        self.played.clear();
        self.current = current_id.and_then(|id| self.position_of(id));
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn current_id(&self) -> Option<&str> {
        self.current
            .and_then(|i| self.ids.get(i))
            .map(|s| s.as_str())
    }

    pub fn position_of(&self, id: &str) -> Option<usize> {
        self.ids.iter().position(|s| s == id)
    }

    pub fn set_current(&mut self, id: &str) {
        self.current = self.position_of(id);
    }

    /// 记录一次成功开播：追加 history（相邻去重、最多 100 条），shuffle 标记本轮已播。
    pub fn mark_played(&mut self, id: &str) {
        if self.mode == PlayMode::Shuffle {
            if let Some(pos) = self.position_of(id) {
                self.played.insert(pos);
            }
        }
        if self.history.last().map(|s| s.as_str()) != Some(id) {
            self.history.push(id.to_string());
            if self.history.len() > HISTORY_CAP {
                let excess = self.history.len() - HISTORY_CAP;
                self.history.drain(0..excess);
            }
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn set_history(&mut self, history: Vec<String>) {
        let start = history.len().saturating_sub(HISTORY_CAP);
        self.history = history[start..].to_vec();
    }

    pub fn next(&mut self, rng: &mut impl Rng) -> Result<usize, QueueError> {
        let len = self.ids.len();
        if len == 0 {
            return Err(QueueError::Empty);
        }
        match self.mode {
            PlayMode::Sequential => match self.current {
                None => {
                    self.current = Some(0);
                    Ok(0)
                }
                Some(i) if i + 1 < len => {
                    self.current = Some(i + 1);
                    Ok(i + 1)
                }
                _ => Err(QueueError::AtEnd),
            },
            PlayMode::Loop => {
                let next = match self.current {
                    None => 0,
                    Some(i) => (i + 1) % len,
                };
                self.current = Some(next);
                Ok(next)
            }
            PlayMode::Shuffle => {
                let candidates: Vec<usize> = (0..len)
                    .filter(|&i| Some(i) != self.current && !self.played.contains(&i))
                    .collect();
                if candidates.is_empty() {
                    // 本轮取完：重置后取除当前台外的任意一台。
                    self.played.clear();
                    let reset: Vec<usize> =
                        (0..len).filter(|&i| Some(i) != self.current).collect();
                    if reset.is_empty() {
                        let idx = self.current.unwrap_or(0);
                        self.current = Some(idx);
                        return Ok(idx);
                    }
                    let pick = reset[rng.gen_range(0..reset.len())];
                    self.current = Some(pick);
                    self.played.insert(pick);
                    Ok(pick)
                } else {
                    let pick = candidates[rng.gen_range(0..candidates.len())];
                    self.current = Some(pick);
                    self.played.insert(pick);
                    Ok(pick)
                }
            }
        }
    }

    pub fn prev(&mut self) -> Result<usize, QueueError> {
        let len = self.ids.len();
        if len == 0 {
            return Err(QueueError::Empty);
        }
        match self.mode {
            PlayMode::Sequential => match self.current {
                None => {
                    self.current = Some(0);
                    Ok(0)
                }
                Some(0) => Err(QueueError::AtEnd),
                Some(i) => {
                    self.current = Some(i - 1);
                    Ok(i - 1)
                }
            },
            PlayMode::Loop => {
                let prev = match self.current {
                    None => 0,
                    Some(i) => (i + len - 1) % len,
                };
                self.current = Some(prev);
                Ok(prev)
            }
            PlayMode::Shuffle => {
                let current_id = self.current_id().map(|s| s.to_string());
                let history = self.history.clone();
                for (idx, id) in history.iter().enumerate().rev() {
                    if current_id.as_deref() == Some(id.as_str()) {
                        continue;
                    }
                    if let Some(pos) = self.position_of(id) {
                        // 回到历史里的上一台后，丢弃它之后的记录，保证连续 prev 能继续往回走。
                        self.history.truncate(idx + 1);
                        self.current = Some(pos);
                        return Ok(pos);
                    }
                }
                Err(QueueError::AtEnd)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn q(ids: &[&str], mode: PlayMode) -> Queue {
        let mut q = Queue::new(mode);
        q.set_ids(ids.iter().map(|s| s.to_string()).collect(), None);
        q
    }

    #[test]
    fn test_sequential_end_and_prev() {
        let mut q = q(&["a", "b", "c"], PlayMode::Sequential);
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Ok(0));
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Ok(1));
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Ok(2));
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Err(QueueError::AtEnd));
        assert_eq!(q.current_id(), Some("c"));
        assert_eq!(q.prev(), Ok(1));
        assert_eq!(q.prev(), Ok(0));
        assert_eq!(q.prev(), Err(QueueError::AtEnd));
    }

    #[test]
    fn test_loop_wraps_both_ends() {
        let mut q = q(&["a", "b", "c"], PlayMode::Loop);
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Ok(0));
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Ok(1));
        q.set_current("c");
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Ok(0));
        assert_eq!(q.prev(), Ok(2));
        assert_eq!(q.prev(), Ok(1));
        assert_eq!(q.prev(), Ok(0));
    }

    #[test]
    fn test_shuffle_no_repeat_until_round_done() {
        let ids: Vec<String> = (0..10).map(|i| format!("s{i}")).collect();
        let mut q = Queue::new(PlayMode::Shuffle);
        q.set_ids(ids, None);
        let mut rng = StdRng::seed_from_u64(42);
        let mut seen = HashSet::new();
        for _ in 0..10 {
            let idx = q.next(&mut rng).unwrap();
            assert!(seen.insert(idx), "本轮内重复了下标 {idx}");
        }
        assert_eq!(seen.len(), 10);
        // 第 11 次应开启新一轮而不是报错。
        assert!(q.next(&mut rng).is_ok());
    }

    #[test]
    fn test_shuffle_prev_uses_history() {
        let mut q = q(&["a", "b", "c"], PlayMode::Shuffle);
        q.set_current("a");
        q.mark_played("a");
        q.set_current("b");
        q.mark_played("b");
        q.set_current("c");
        q.mark_played("c");
        assert_eq!(q.prev(), Ok(1));
        assert_eq!(q.prev(), Ok(0));
        assert_eq!(q.prev(), Err(QueueError::AtEnd));
    }

    #[test]
    fn test_empty_scope() {
        let mut q = q(&[], PlayMode::Loop);
        assert_eq!(q.next(&mut StdRng::seed_from_u64(1)), Err(QueueError::Empty));
        assert_eq!(q.prev(), Err(QueueError::Empty));
    }

    #[test]
    fn test_set_ids_keeps_current_when_present() {
        let mut q = q(&["a", "b", "c"], PlayMode::Sequential);
        q.set_current("b");
        q.set_ids(vec!["b".into(), "d".into()], Some("b"));
        assert_eq!(q.current_id(), Some("b"));
        q.set_ids(vec!["x".into(), "y".into()], Some("b"));
        assert_eq!(q.current_id(), None);
    }

    #[test]
    fn test_history_capped_at_100() {
        let mut q = Queue::new(PlayMode::Loop);
        for i in 0..150 {
            q.mark_played(&format!("s{i}"));
        }
        assert_eq!(q.history().len(), HISTORY_CAP);
        assert_eq!(q.history()[0], "s50");
        assert_eq!(q.history()[HISTORY_CAP - 1], "s149");
    }

    #[test]
    fn test_mark_played_dedupes_adjacent() {
        let mut q = Queue::new(PlayMode::Loop);
        q.mark_played("a");
        q.mark_played("a");
        q.mark_played("b");
        assert_eq!(q.history(), &["a", "b"]);
    }
}
