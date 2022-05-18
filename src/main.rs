use image::GenericImageView;

type Map<K, V> = std::collections::BTreeMap<K, V>;

trait DirsExt {
    fn all_dirs(&self) -> &'static [dmm_tools::dmi::Dir];
}
impl DirsExt for dmm_tools::dmi::Dirs {
    fn all_dirs(&self) -> &'static [dmm_tools::dmi::Dir] {
        match self {
            dmm_tools::dmi::Dirs::One => &[dmm_tools::dmi::Dir::South],
            dmm_tools::dmi::Dirs::Four => dmm_tools::dmi::Dir::CARDINALS,
            dmm_tools::dmi::Dirs::Eight => dmm_tools::dmi::Dir::ALL,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum FragmentStatus {
    Added,
    Removed,
    Unchanged,
    ChangedOffsetOnly, // benign
    ChangedMeta,       // and maybe pixels too
    ChangedPixels,
}
impl FragmentStatus {
    fn set_pixel_changed(&mut self, changed: bool) {
        assert_eq!(changed, true);
        match &self {
            Self::Unchanged => *self = Self::ChangedPixels,
            Self::ChangedOffsetOnly => *self = Self::ChangedMeta,
            _ => todo!(),
        }
    }
    fn is_meaningful_change(&self) -> bool {
        use FragmentStatus::*;
        match self {
            Unchanged | ChangedOffsetOnly => false,
            _ => true,
        }
    }
    fn has_meta_change(&self) -> bool {
        use FragmentStatus::*;
        match self {
            Unchanged | ChangedPixels => false,
            _ => true,
        }
    }
    fn has_pixel_change(&self) -> bool {
        use FragmentStatus::*;
        match self {
            Unchanged | ChangedOffsetOnly => false,
            _ => true,
        }
    }
}

fn dmm_img_to_image(
    bitmap: &lodepng::Bitmap<lodepng::RGBA>,
) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    let img = image::ImageBuffer::from_fn(bitmap.width as u32, bitmap.height as u32, |x, y| {
        let v = bitmap.buffer[(y as usize * bitmap.width) + x as usize];
        image::Rgba([v.r, v.g, v.b, v.a])
    });

    img
}

fn fragment_report(src_bytes: &[u8], dst_bytes: &[u8]) -> Map<String, FragmentStatus> {
    let (img_raw_src, meta_src) = dreammaker::dmi::Metadata::from_bytes(src_bytes).unwrap();
    let (img_raw_dst, meta_dst) = dreammaker::dmi::Metadata::from_bytes(dst_bytes).unwrap();

    let img_src = dmm_img_to_image(&img_raw_src);
    let img_dst = dmm_img_to_image(&img_raw_dst);

    let all_state_names = std::iter::empty()
        .chain(meta_src.state_names.keys())
        .chain(meta_dst.state_names.keys())
        .collect::<std::collections::BTreeSet<_>>();

    let mut report = Map::new();
    'state_loop: for state_name in all_state_names.into_iter().cloned() {
        let mut preliminary_status = FragmentStatus::Unchanged;

        let state_src = &meta_src.states[match meta_src.state_names.get(&state_name) {
            Some(i) => *i,
            None => {
                report.insert(state_name, FragmentStatus::Added);
                continue;
            }
        }];
        let state_dst = &meta_dst.states[match meta_dst.state_names.get(&state_name) {
            Some(i) => *i,
            None => {
                report.insert(state_name, FragmentStatus::Removed);
                continue;
            }
        }];
        if state_src != state_dst {
            let mut state_copy = state_src.clone();
            state_copy.offset = state_dst.offset;
            if &state_copy != state_dst {
                println!("Was: {:?}\nNew: {:?}", state_src, state_dst);
                report.insert(state_name, FragmentStatus::ChangedMeta);
                continue;
            } // else
            preliminary_status = FragmentStatus::ChangedOffsetOnly
        }
        let state = state_src;

        for dir in state.dirs.all_dirs() {
            for frame in 0..state.frames.count() {
                let rect_src = meta_src
                    .rect_of(img_raw_src.width as u32, &state_name, *dir, frame as u32)
                    .unwrap();
                let rect_dst = meta_dst
                    .rect_of(img_raw_dst.width as u32, &state_name, *dir, frame as u32)
                    .unwrap();

                let view_src = img_src
                    .view(rect_src.0, rect_src.1, rect_src.2, rect_src.3)
                    .to_image();
                let view_dst = img_dst
                    .view(rect_dst.0, rect_dst.1, rect_dst.2, rect_dst.3)
                    .to_image();
                if view_src != view_dst {
                    preliminary_status.set_pixel_changed(true);
                    report.insert(state_name, preliminary_status);
                    continue 'state_loop;
                }
            }
        }

        report.insert(state_name, preliminary_status);
    }

    return report;
}

fn incorporate_pixel_changes(
    bytes_base: &[u8],
    bytes_inc: &[u8],
    states: &[String],
) -> anyhow::Result<Vec<u8>> {
    let (img_raw_base, meta_base) = dreammaker::dmi::Metadata::from_bytes(bytes_base).unwrap();
    let (img_raw_inc, meta_inc) = dreammaker::dmi::Metadata::from_bytes(bytes_inc).unwrap();

    let mut img_base = dmm_img_to_image(&img_raw_base);
    let img_inc = dmm_img_to_image(&img_raw_inc);

    for state_name in states {
        let state_base = &meta_base.states[meta_base.state_names[state_name]];
        // let state_inc = &meta_inc.states[meta_base.state_names[state_name]];

        for dir in state_base.dirs.all_dirs() {
            for frame in 0..state_base.frames.count() {
                use image::GenericImage;
                let rect_base = meta_base
                    .rect_of(img_raw_base.width as u32, &state_name, *dir, frame as u32)
                    .unwrap();
                let rect_inc = meta_inc
                    .rect_of(img_raw_inc.width as u32, &state_name, *dir, frame as u32)
                    .unwrap();

                let mut view_base =
                    img_base.sub_image(rect_base.0, rect_base.1, rect_base.2, rect_base.3);
                let view_inc = img_inc
                    .view(rect_inc.0, rect_inc.1, rect_inc.2, rect_inc.3)
                    .to_image();
                for (x, y, pixel) in view_inc.enumerate_pixels() {
                    view_base.put_pixel(x, y, pixel.clone())
                }
            }
        }
    }

    // now... reparse the file again to extract the tags, and then reencode it.
    let decoder = png::Decoder::new(bytes_base);
    let reader = decoder.read_info()?;
    let info = reader.info();

    // let (indexed_image, palette) = index_colors(img_base.as_flat_samples().image_slice().unwrap());
    let mut out_bytes = vec![];
    let mut encoder = png::Encoder::new(&mut out_bytes, info.width, info.height);
    for z in &info.compressed_latin1_text {
        encoder.add_ztxt_chunk(z.keyword.to_string(), z.get_text()?)?;
    }
    // encoder.set_palette(palette);
    // encoder.set_depth(png::BitDepth::Eight);
    encoder.set_color(png::ColorType::Rgba);
    let mut w = encoder.write_header()?;
    w.write_image_data(&img_base.as_flat_samples().image_slice().unwrap())
        .unwrap();
    w.finish().unwrap();

    Ok(out_bytes)
}

fn merge_stuff(repo_path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
    let repo_path = repo_path.as_ref();
    let repo = git2::Repository::open(repo_path).unwrap();

    // path -> success
    let mut conflict_paths = std::collections::BTreeMap::new();
    'conflict_loop: for conflict in repo
        .index()
        .unwrap()
        .conflicts()
        .unwrap()
        .map(Result::unwrap)
    {
        let conflict_path = std::str::from_utf8(&conflict.our.as_ref().unwrap().path)
            .unwrap()
            .to_owned();
        conflict_paths.insert(conflict_path.clone(), false);
        println!("merging {:?}..", conflict_path);
        let blob_a = repo.find_blob(conflict.ancestor.unwrap().id).unwrap();
        let blob_o = repo.find_blob(conflict.our.unwrap().id).unwrap();
        let blob_t = repo.find_blob(conflict.their.unwrap().id).unwrap();

        let report_our = fragment_report(blob_a.content(), blob_o.content());
        let report_their = fragment_report(blob_a.content(), blob_t.content());
        println!("  our changes:");
        for (k, v) in report_our.iter().filter(|(_, v)| v.is_meaningful_change()) {
            println!("    {} - {:?}", k, v);
        }
        println!("  their changes:");
        for (k, v) in report_their
            .iter()
            .filter(|(_, v)| v.is_meaningful_change())
        {
            println!("    {} - {:?}", k, v);
        }

        let all_keys = std::iter::empty()
            .chain(report_our.keys())
            .chain(report_their.keys())
            .cloned()
            .collect::<Vec<_>>();

        for s in all_keys {
            if report_our[&s].has_pixel_change() && report_their[&s].has_pixel_change() {
                println!("  Both we and they changed the state {:?}. Can't merge", s);
                continue 'conflict_loop;
            }
        }

        // find suitable base
        let changed_meta_o = report_our.values().any(|s| s.has_meta_change());
        let changed_meta_t = report_their.values().any(|s| s.has_meta_change());
        let out_path = repo_path.join(&conflict_path);
        match (changed_meta_o, changed_meta_t) {
            (true, true) => {
                println!("  Both we and they changed the file dmi metadata. Cant merge the changes without messing things up");
                continue 'conflict_loop;
            }
            (true, false) | (false, false) => {
                // use us as base
                println!("  Using our file as a base..");
                let states_to_overlay = report_their.keys().cloned().collect::<Vec<_>>();
                let new_content = incorporate_pixel_changes(
                    blob_o.content(),
                    blob_t.content(),
                    &states_to_overlay,
                )?;

                std::io::Write::write_all(
                    &mut std::fs::OpenOptions::new()
                        .create(true)
                        .truncate(true)
                        .write(true)
                        .open(out_path)
                        .unwrap(),
                    &new_content,
                )
                .unwrap();
            }
            (false, true) => {
                // use them as base
                println!("  Using their file as a base..");
                let states_to_overlay = report_our.keys().cloned().collect::<Vec<_>>();
                let new_content = incorporate_pixel_changes(
                    blob_t.content(),
                    blob_o.content(),
                    &states_to_overlay,
                )?;
                std::io::Write::write_all(
                    &mut std::fs::OpenOptions::new()
                        .create(true)
                        .truncate(true)
                        .write(true)
                        .open(out_path)
                        .unwrap(),
                    &new_content,
                )
                .unwrap();
            }
        }
        println!("  Success");
        *conflict_paths.get_mut(&conflict_path).unwrap() = true;
    }

    if conflict_paths.is_empty() {
        println!("There are no conflicts in the repository. Try merging or rebasing to get them.")
    }

    let good = conflict_paths.iter().filter(|x| *x.1).collect::<Vec<_>>();
    let failed = conflict_paths.iter().filter(|x| !x.1).collect::<Vec<_>>();

    if !good.is_empty() {
        println!(
        "\nThe dmi files were (hopefully) deconflicted, but the format we saved them in is very unoptimized."
    );
        println!(
            "It is *STRONGLY* advised that you resave the now-deconflicted files in dream maker."
        );
        println!("After you do that, mark the merge conflicts as resolved in git, and commit the merge, as you would normally do.");
        println!("Here's the list of files, for reference:");
        for (conflict_path, _) in conflict_paths.iter().filter(|(_, v)| **v) {
            println!("  {}", conflict_path)
        }
    }

    if !failed.is_empty() {
        println!("\nThese are the files we failed to deconflict automatically:");
        for (f, _) in failed {
            println!("  {}", f)
        }
    }
    Ok(())
}

fn main() {
    let repo_path = std::env::args().nth(1).unwrap_or_else(|| {
        println!("provide the path to the repository");
        std::process::exit(1)
    });
    merge_stuff(repo_path).unwrap();
}
