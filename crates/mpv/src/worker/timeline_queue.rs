//! Fold adjacent timeline updates without crossing another command or media boundary.
use super::Message;
use riina_core::Command;
use std::sync::mpsc::Receiver;

pub(super) fn latest(
    mut message: Message,
    inbox: &Receiver<Message>,
) -> (Message, Option<Message>) {
    let Message::MediaIntent {
        generation,
        command: Command::SeekTimeline { .. },
    } = message
    else {
        return (message, None);
    };
    // Bound one pass so ongoing input cannot starve native event processing.
    for _ in 0..16 {
        let Ok(next) = inbox.try_recv() else { break };
        match next {
            Message::MediaIntent {
                generation: current,
                command: Command::SeekTimeline { .. },
            } if current == generation => message = next,
            _ => return (message, Some(next)),
        }
    }
    (message, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn seek(generation: u64, fraction: f64, exact: bool) -> Message {
        Message::MediaIntent {
            generation,
            command: Command::SeekTimeline { fraction, exact },
        }
    }

    #[test]
    fn folds_only_adjacent_requests_for_the_same_media() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, inbox) = channel();
        for message in [
            seek(7, 0.3, true),
            seek(7, 0.5, false),
            Message::Intent(Command::Stop(None)),
            seek(7, 0.8, true),
            seek(8, 0.1, false),
            Message::Shutdown,
        ] {
            sender.send(message)?;
        }
        let (message, next) = latest(seek(7, 0.1, true), &inbox);
        assert!(matches!(
            message,
            Message::MediaIntent {
                generation: 7,
                command: Command::SeekTimeline {
                    fraction: 0.5,
                    exact: false
                },
            }
        ));
        assert!(matches!(next, Some(Message::Intent(Command::Stop(None)))));
        let (message, next) = latest(inbox.recv()?, &inbox);
        assert!(matches!(
            message,
            Message::MediaIntent {
                generation: 7,
                command: Command::SeekTimeline {
                    fraction: 0.8,
                    exact: true
                },
            }
        ));
        let (message, next) = latest(next.ok_or("Expected the replacement's seek")?, &inbox);
        assert!(matches!(
            message,
            Message::MediaIntent {
                generation: 8,
                command: Command::SeekTimeline {
                    fraction: 0.1,
                    exact: false
                },
            }
        ));
        assert!(matches!(next, Some(Message::Shutdown)));
        Ok(())
    }
}
