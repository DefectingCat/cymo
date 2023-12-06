use std::path::PathBuf;

/// Find parents of all files
///
/// ## Arguments
///
/// `local_path`: local files path from params
///
/// ## Return
///
/// `Fn`: used for `fold()`'s callback
pub fn fold_parents(local_path: &String) -> impl Fn(Vec<PathBuf>, &PathBuf) -> Vec<PathBuf> {
    let local_path = PathBuf::from(local_path);

    move |mut prev: Vec<_>, cur: &PathBuf| -> Vec<PathBuf> {
        let skip_count = local_path
            .parent()
            .unwrap_or(&PathBuf::new())
            .components()
            .count();
        let skip_count = if local_path.is_dir() && local_path.components().count() == 1 {
            1
        } else {
            skip_count
        };
        let parent = cur
            .parent()
            .map(|parent| parent.components().skip(skip_count))
            .filter(|p| p.clone().count() > 0)
            .map(|p| p.collect::<PathBuf>());
        if let Some(p) = parent {
            if prev.contains(&p) {
                return prev;
            }
            let components = p.components().collect::<Vec<_>>();
            for index in 1..=components.len() {
                let path = &components[..index]
                    .iter()
                    .fold(PathBuf::new(), |mut child, cur| {
                        child.push(PathBuf::from(cur));
                        child
                    });
                prev.push(path.clone());
            }
        }
        prev
    }
}
