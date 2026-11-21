pub(super) fn online_videos(
    preferences: &crate::network_preferences::OnlineVideos,
) -> Result<Vec<Vec<String>>, String> {
    Ok(preferences
        .engine_options()?
        .into_iter()
        .map(|(name, value)| super::set(&name, value))
        .collect())
}
