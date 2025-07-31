pub trait PullState {
    fn pull_state_from(&mut self, source: &Self) {
        // Do nothing by default
        let _ = source;
    }
}

impl<T: Clone> PullState for T {
    fn pull_state_from(&mut self, source: &Self) {
        self.clone_from(source);
    }
}
