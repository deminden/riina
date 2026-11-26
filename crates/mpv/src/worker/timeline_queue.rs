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
        command: Command::SeekTimeline(_),
        filter_name: None,
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
                command: Command::SeekTimeline(_),
                filter_name: None,
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

    fn seek(generation: u64, fraction: f64) -> Message {
        Message::MediaIntent {
            generation,
            command: Command::SeekTimeline(fraction),
            filter_name: None,
        }
    }

    #[test]
    fn folds_only_adjacent_requests_for_the_same_media() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, inbox) = channel();
        for message in [
            seek(7, 0.3),
            seek(7, 0.5),
            Message::Intent(Command::Stop),
            seek(7, 0.8),
            seek(8, 0.1),
            Message::Shutdown,
        ] {
            sender.send(message)?;
        }
        let (message, next) = latest(seek(7, 0.1), &inbox);
        assert!(matches!(
            message,
            Message::MediaIntent {
                generation: 7,
                command: Command::SeekTimeline(0.5),
                filter_name: None,
            }
        ));
        assert!(matches!(next, Some(Message::Intent(Command::Stop))));
        let (message, next) = latest(inbox.recv()?, &inbox);
        assert!(matches!(
            message,
            Message::MediaIntent {
                generation: 7,
                command: Command::SeekTimeline(0.8),
                filter_name: None,
            }
        ));
        let (message, next) = latest(next.ok_or("Expected the replacement's seek")?, &inbox);
        assert!(matches!(
            message,
            Message::MediaIntent {
                generation: 8,
                command: Command::SeekTimeline(0.1),
                filter_name: None,
            }
        ));
        assert!(matches!(next, Some(Message::Shutdown)));
        Ok(())
    }
}
