use tokio::sync::mpsc;

pub enum Command {
    Rollout,
    Feedback,
    Stop,
}

pub enum Event {
    RolloutSchedule,
    RolloutDone,
}

pub async fn handle_command(cmd: &Command, state: &mut usize) -> eyre::Result<()> {
    match cmd {
        Command::Rollout => *state = *state + 1,
        Command::Feedback => *state = *state + 2,
        Command::Stop => *state = 0,
    }
    Ok(())
}

pub async fn demo() -> eyre::Result<()> {
    let (_cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(1);
    let (_event_tx, _event_rx) = mpsc::channel::<Event>(1);
    let mut state = 0usize;
    let mut command: Option<Command> = None;
    loop {
        match command {
            None => command = cmd_rx.recv().await,
            Some(ref cmd) => tokio::select! {
                res = handle_command(cmd, &mut state) => {return res;},
                new_cmd = cmd_rx.recv() => {command = new_cmd;},
            },
        }
    }
}
