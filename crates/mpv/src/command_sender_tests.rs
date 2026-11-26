use super::{Command, CommandQueue, CommandSender, EngineError, Message, Owner, WakeupContext};
use std::rc::Rc;
use std::sync::{atomic::Ordering, mpsc};

#[derive(Debug, PartialEq)]
enum Expected {
    Intent(Option<u64>, Command),
    Wake,
}

#[test]
fn mixed_intents_and_wakes_match_bounded_fifo_model() {
    let (done, result) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        for capacity in [1, 2, 64] {
            for seed in [1_u64, 17, 0x5eed] {
                let (sender, receiver) = mpsc::sync_channel(capacity);
                let commands = CommandSender {
                    sender: CommandQueue::Native(sender.clone()),
                    stopped: Default::default(),
                };
                let wakeup = WakeupContext {
                    sender,
                    queued: std::sync::atomic::AtomicBool::new(false),
                };
                let mut model = std::collections::VecDeque::new();
                let mut random = seed;
                let mut accepted = 0;
                let mut rejected = 0;
                let mut delivered_wakes = 0;
                for step in 0..4096 {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let action = (random >> 32) % 8;
                    let context = format!("capacity {capacity}, seed {seed}, step {step}");
                    match action {
                        0..=3 => {
                            let command = match action {
                                0 => Command::TogglePause,
                                1 => Command::SetPaused(step % 2 == 0),
                                2 => Command::Volume(f64::from(step % 101)),
                                _ => Command::Seek {
                                    seconds: f64::from(step),
                                    absolute: true,
                                },
                            };
                            let generation = (action == 3).then_some(random);
                            let sent = match command {
                                Command::TogglePause => commands.toggle_pause(),
                                Command::SetPaused(value) => commands.set_paused(value),
                                _ if let Some(generation) = generation => {
                                    commands.send_for_media(generation, command.clone())
                                }
                                _ => commands.send(command.clone()),
                            };
                            let should_accept = model.len() < capacity;
                            assert_eq!(sent.is_ok(), should_accept, "{context}: admission");
                            if should_accept {
                                model.push_back(Expected::Intent(generation, command));
                                accepted += 1;
                            } else {
                                rejected += 1;
                            }
                        }
                        4 => {
                            if !model.contains(&Expected::Wake) && model.len() < capacity {
                                model.push_back(Expected::Wake);
                            }
                            // A burst must coalesce without displacing accepted input.
                            for _ in 0..3 {
                                wakeup.signal();
                            }
                        }
                        _ => match (receiver.try_recv(), model.pop_front()) {
                            (
                                Ok(Message::Intent(actual)),
                                Some(Expected::Intent(None, expected)),
                            ) => {
                                assert_eq!(actual, expected, "{context}: FIFO")
                            }
                            (
                                Ok(Message::MediaIntent {
                                    generation,
                                    command,
                                }),
                                Some(Expected::Intent(Some(expected_generation), expected)),
                            ) => {
                                assert_eq!(
                                    (generation, command),
                                    (expected_generation, expected),
                                    "{context}: FIFO"
                                );
                            }
                            (Ok(Message::Wake), Some(Expected::Wake)) => {
                                wakeup.queued.store(false, Ordering::Release);
                                delivered_wakes += 1;
                            }
                            (Err(mpsc::TryRecvError::Empty), None) => {}
                            _ => panic!("{context}: delivered an unexpected/missing message"),
                        },
                    }
                    assert_eq!(
                        wakeup.queued.load(Ordering::Acquire),
                        model.contains(&Expected::Wake),
                        "{context}: wake rearming"
                    );
                }
                assert!(
                    accepted > 0 && rejected > 0 && delivered_wakes > 0,
                    "model must exercise acceptance, saturation and wake delivery"
                );
                drop(commands);
                drop(wakeup);
                for expected in model {
                    let actual = match receiver
                        .recv_timeout(std::time::Duration::from_secs(1))
                        .unwrap()
                    {
                        Message::Intent(command) => Expected::Intent(None, command),
                        Message::MediaIntent {
                            generation,
                            command,
                        } => Expected::Intent(Some(generation), command),
                        Message::Wake => Expected::Wake,
                        Message::Client(_)
                        | Message::FilterEdit(_)
                        | Message::OpenStream(..)
                        | Message::PlaylistEdit(_)
                        | Message::RenderingReady => panic!("Unexpected request"),
                        Message::Shutdown => panic!("Command senders cannot shut down the owner"),
                    };
                    assert_eq!(
                        actual, expected,
                        "capacity {capacity}, seed {seed}: final drain"
                    );
                }
                assert!(
                    matches!(
                        receiver.recv_timeout(std::time::Duration::from_secs(1)),
                        Err(mpsc::RecvTimeoutError::Disconnected)
                    ),
                    "no rejected input may be delivered later by a hidden sender"
                );
            }
        }
        done.send(()).unwrap();
    });
    result
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("command send blocked or model worker failed");
    worker.join().unwrap();
}

#[test]
fn full_queue_rejects_pause_promptly_and_preserves_accepted_order() {
    let (sender, receiver) = mpsc::sync_channel(3);
    let command_sender = CommandSender {
        sender: CommandQueue::Native(sender),
        stopped: Default::default(),
    };
    command_sender.set_paused(true).unwrap();
    command_sender.toggle_pause().unwrap();
    command_sender.set_paused(false).unwrap();
    let (done, result) = mpsc::sync_channel(1);
    let caller = std::thread::spawn(move || {
        done.send(command_sender.toggle_pause()).unwrap();
        command_sender
    });
    let error = result
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, EngineError::QueueFull));
    assert_eq!(
        error.to_string(),
        "Player command was not accepted: sending on a full channel"
    );
    let command_sender = caller.join().unwrap();
    for expected in [
        Command::SetPaused(true),
        Command::TogglePause,
        Command::SetPaused(false),
    ] {
        match receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
        {
            Message::Intent(actual) => assert_eq!(actual, expected),
            Message::Wake => panic!("Unexpected wake in intent queue"),
            Message::Client(_)
            | Message::OpenStream(..)
            | Message::MediaIntent { .. }
            | Message::FilterEdit(_)
            | Message::PlaylistEdit(_)
            | Message::RenderingReady => {
                panic!("Unexpected request")
            }
            Message::Shutdown => panic!("Command senders cannot shut down the owner"),
        }
    }
    assert!(receiver.try_recv().is_err());
    command_sender.set_paused(true).unwrap();
    assert!(matches!(
        receiver.recv_timeout(std::time::Duration::from_secs(1)),
        Ok(Message::Intent(Command::SetPaused(true)))
    ));
    drop(receiver);
    let error = command_sender.toggle_pause().unwrap_err();
    assert!(matches!(error, EngineError::Stopped));
    assert_eq!(error.to_string(), "The playback engine has stopped");
}

#[test]
fn native_wake_notifications_share_one_queued_message() {
    let (sender, receiver) = mpsc::sync_channel(64);
    let wakeup = WakeupContext {
        sender,
        queued: std::sync::atomic::AtomicBool::new(false),
    };
    for _ in 0..100 {
        wakeup.signal();
    }
    assert!(matches!(receiver.try_recv(), Ok(Message::Wake)));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn full_intent_queue_does_not_leave_wakeup_permanently_armed() {
    let (sender, receiver) = mpsc::sync_channel(1);
    sender.try_send(Message::Intent(Command::Stop)).unwrap();
    let wakeup = WakeupContext {
        sender,
        queued: std::sync::atomic::AtomicBool::new(false),
    };
    wakeup.signal();
    assert!(!wakeup.queued.load(Ordering::Acquire));
    assert!(matches!(
        receiver.recv(),
        Ok(Message::Intent(Command::Stop))
    ));

    wakeup.signal();
    assert!(matches!(receiver.recv(), Ok(Message::Wake)));
}

#[test]
fn public_shutdown_is_rejected_without_consuming_a_command_slot() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let commands = CommandSender {
        sender: CommandQueue::Native(sender),
        stopped: Default::default(),
    };
    assert!(
        commands
            .send(Command::Shutdown)
            .unwrap_err()
            .to_string()
            .contains("final owner")
    );
    commands.send(Command::Stop).unwrap();
    assert!(matches!(
        receiver.try_recv(),
        Ok(Message::Intent(Command::Stop))
    ));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn only_the_final_owner_delivers_shutdown_even_when_command_senders_survive() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let commands = CommandSender {
        sender: CommandQueue::Native(sender.clone()),
        stopped: Default::default(),
    };
    let owner = Rc::new(Owner {
        sender,
        stopped: commands.stopped.clone(),
        thread: None,
    });
    let renderer_owner = owner.clone();
    drop(owner);
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(renderer_owner);
    assert!(matches!(receiver.try_recv(), Ok(Message::Shutdown)));
    drop(receiver);
    assert!(commands.send(Command::Stop).is_err());
}
