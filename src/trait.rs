pub(super) trait Execute {
    fn execute(self) -> anyhow::Result<()>;
}
