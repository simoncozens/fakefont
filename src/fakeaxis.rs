use indexmap::IndexMap;
use rand::RngExt as _;
use babelfont::{Axis, DesignCoord, DesignLocation, LayerType::DefaultForMaster, SmolStr, Tag};

use crate::FakeFont;

impl FakeFont {
    /// Creates additional master locations for the fake font based on the specified axes, filling out the corners of the design space.
    ///
    /// This must be called after all axes have been added to the fake font and before the font is compiled.
    pub fn fill_out_masters(&mut self, kerning: bool, advance_widths: bool) {
        // Make first master the default
        let mut default_location = DesignLocation::default();
        for axis in &self.0.axes {
            default_location.insert(
                axis.tag,
                axis.default
                    .map(|uc| DesignCoord::new(uc.to_f64()))
                    .unwrap_or(DesignCoord::new(0.0)),
            );
        }
        self.0.masters[0].location = default_location;
        // Cartesian product top, default, and bottom of all axes to find corner master locations
        let mut corner_locations = vec![];
        for axis in &self.0.axes {
            let min = axis
                .min
                .map(|uc| DesignCoord::new(uc.to_f64()))
                .unwrap_or(DesignCoord::new(0.0));
            let max = axis
                .max
                .map(|uc| DesignCoord::new(uc.to_f64()))
                .unwrap_or(DesignCoord::new(0.0));
            let default = axis
                .default
                .map(|uc| DesignCoord::new(uc.to_f64()))
                .unwrap_or(DesignCoord::new(0.0));
            if corner_locations.is_empty() {
                corner_locations.push({
                    let mut loc = DesignLocation::default();
                    loc.insert(axis.tag, min);
                    loc
                });
                corner_locations.push({
                    let mut loc = DesignLocation::default();
                    loc.insert(axis.tag, max);
                    loc
                });
                corner_locations.push({
                    let mut loc = DesignLocation::default();
                    loc.insert(axis.tag, default);
                    loc
                });
            } else {
                let mut new_corners = vec![];
                for loc in &corner_locations {
                    let mut loc_min = loc.clone();
                    loc_min.insert(axis.tag, min);
                    new_corners.push(loc_min);
                    let mut loc_max = loc.clone();
                    loc_max.insert(axis.tag, max);
                    new_corners.push(loc_max);
                    let mut loc_default = loc.clone();
                    loc_default.insert(axis.tag, default);
                    new_corners.push(loc_default);
                }
                corner_locations = new_corners;
            }
        }
        for loc in corner_locations {
            // Skip if we already have one.
            if self.0.masters.iter().any(|m| m.location == loc) {
                continue;
            }
            let mut new_master = babelfont::Master {
                name: format!("{:?}", loc).into(),
                id: format!("{:?}", loc),
                location: loc,
                kerning: self.0.masters[0].kerning.clone(),
                metrics: self.0.masters[0].metrics.clone(),
                ..Default::default()
            };
            // Create layers, adjust glyph positions
            let axes = &self.0.axes;
            for glyph in self.0.glyphs.iter_mut() {
                if let Some(layer) = glyph.layers.first() {
                    let mut new_layer = layer.clone();
                    new_layer.master = DefaultForMaster(new_master.id.clone());
                    adjust_layer(&mut new_layer, axes, &new_master.location, advance_widths);
                    glyph.layers.push(new_layer);
                }
            }
            if kerning {
                adjust_kerning(&mut new_master.kerning, axes, &new_master.location);
            }
            self.0.masters.push(new_master);
        }
    }

    /// How many masters the designspace has been filled out with.
    pub fn master_count(&self) -> usize {
        self.0.masters.len()
    }

    /// How many glyphs the font currently holds.
    pub fn glyph_count(&self) -> usize {
        self.0.glyphs.len()
    }


}

fn adjust_kerning(
    kerning: &mut IndexMap<(SmolStr, SmolStr), i16>,
    axes: &[Axis],
    loc: &DesignLocation,
) {
    // Drop axes which don't affect kerning
    let mut loc = loc.clone();
    loc.retain(|tag, _| {
        if let Some(axis) = axes.iter().find(|a| a.tag == *tag)
            && let Some(affects_kerning) = axis.format_specific.get("affects_kerning")
        {
            return affects_kerning.as_bool().unwrap_or(true);
        }
        true
    });
    kerning.iter_mut().for_each(|((_left, _right), value)| {
        *value = transform_x_coord(*value, &loc, *value) as i16;
    });
}

fn adjust_layer(
    layer: &mut babelfont::Layer,
    axes: &[Axis],
    loc: &DesignLocation,
    advance_widths: bool,
) {
    let Ok(bounds) = layer.bounds() else { return };
    let midpoint_x = bounds.center().x;
    let midpoint_y = bounds.center().y;

    for shape in layer.shapes.iter_mut() {
        match shape {
            babelfont::Shape::Component(component) => {
                component.transform.translation = (
                    transform_x_coord(component.transform.translation.0, loc, midpoint_x),
                    component.transform.translation.1,
                );
            }
            babelfont::Shape::Path(path) => {
                for point in path.nodes.iter_mut() {
                    point.x = transform_x_coord(point.x, loc, midpoint_x);
                    point.y = transform_y_coord(point.y, loc, midpoint_y);
                }
            }
        }
    }
    for anchor in layer.anchors.iter_mut() {
        anchor.x = transform_x_coord(anchor.x, loc, midpoint_x);
    }
    if advance_widths {
        let mut loc = loc.clone();
        // let axes = &font.axes;
        // Remove any axes which don't affect metrics
        loc.retain(|tag, _| {
            if let Some(axis) = axes.iter().find(|a| a.tag == *tag)
                && let Some(affects_metrics) = axis.format_specific.get("affects_metrics")
            {
                return affects_metrics.as_bool().unwrap_or(true);
            }
            true
        });
        layer.width = transform_x_coord(layer.width, &loc, midpoint_x) as f32;
    }
}

fn transform_x_coord(x: impl Into<f64>, loc: &DesignLocation, midpoint: impl Into<f64>) -> f64 {
    let mut rng = rand::rng();
    let mut x = x.into();
    if let Some(weight) = loc.get(Tag::new(b"wght")) {
        let factor = (weight.to_f64() - 400.0) / 800.0;
        // Push points toward (or away from) midpoint
        x += (x - midpoint.into()) * factor;
    }
    if let Some(width) = loc.get(Tag::new(b"wdth")) {
        x *= width.to_f64() / 100.0;
    }
    // Look for other axes
    for (tag, value) in loc.iter() {
        if tag != &Tag::new(b"wght") && tag != &Tag::new(b"wdth") {
            x += rng.random_range(value.to_f64() - 5.0..value.to_f64() + 5.0);
        }
    }
    // Jitter slightly to push up gvar
    x += rng.random_range(-2.0..5.0);

    x
}

fn transform_y_coord(y: impl Into<f64>, loc: &DesignLocation, midpoint: impl Into<f64>) -> f64 {
    let mut rng = rand::rng();
    let mut y = y.into();
    if let Some(weight) = loc.get(Tag::new(b"wght")) {
        let factor = (weight.to_f64() - 400.0) / 800.0;
        // Push points toward (or away from) midpoint
        y += (y - midpoint.into()) * factor;
    }
    for (tag, value) in loc.iter() {
        if tag != &Tag::new(b"wght") && tag != &Tag::new(b"wdth") {
            y += rng.random_range(value.to_f64() - 5.0..value.to_f64() + 5.0);
        }
    }
    y += rng.random_range(-2.0..5.0);
    y
}