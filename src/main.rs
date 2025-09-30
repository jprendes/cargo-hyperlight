use anyhow::Result;
use cargo_hyperlight::Command;

fn main() -> Result<()> {
    Command::parse()?.run()
}
