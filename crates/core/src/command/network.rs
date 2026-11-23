use super::CommandError;
pub(super) fn online_videos(
    preferences: &crate::network_preferences::OnlineVideos,
) -> Result<Vec<Vec<String>>, CommandError> {
    Ok(preferences
        .engine_options()
        .map_err(CommandError::Preference)?
        .into_iter()
        .map(|(name, value)| super::set(&name, value))
        .collect())
}
