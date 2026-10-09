//! Paint Bucket and Gradient keep alpha on a transparency-locked layer, as Edit › Fill and the
//! brushes do (#1104).

use photocraft_engine::Session;
use serde_json::json;

/// A transparent layer with a red stroke across it, transparency locked.
fn locked() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 40, "height": 30})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.execute("paint.stroke", json!({"points": [[5, 10], [20, 10]], "size": 8, "hardness": 1.0, "color": "#ff0000"})).unwrap();
    s.execute("layer.lockLayers", json!({"transparency": true})).unwrap();
    s
}

fn px(s: &Session, x: i32, y: i32) -> [f32; 4] {
    let st = s.active().unwrap();
    st.active_layer.and_then(|id| st.doc.layer(id)).unwrap().surface().unwrap().rgba(x, y)
}

#[test]
fn bucket_keeps_alpha() {
    let mut s = locked();
    s.execute("paint.bucket", json!({"x": 30, "y": 25, "color": "#0000ff", "antiAlias": false})).unwrap();
    assert_eq!(px(&s, 30, 25)[3], 0.0, "transparent pixel became opaque");
    s.execute("paint.bucket", json!({"x": 12, "y": 10, "color": "#0000ff", "antiAlias": false})).unwrap();
    assert_eq!(px(&s, 12, 10), [0.0, 0.0, 1.0, 1.0], "the opaque stroke is still recoloured");
}

#[test]
fn gradient_keeps_alpha() {
    let mut s = locked();
    s.execute("paint.gradient", json!({"from": [0, 0], "to": [40, 30]})).unwrap();
    assert_eq!(px(&s, 30, 25)[3], 0.0, "transparent pixel became opaque");
    assert_eq!(px(&s, 12, 10)[3], 1.0);
    assert_ne!(px(&s, 12, 10), [1.0, 0.0, 0.0, 1.0], "the opaque stroke is still painted");
}

#[test]
fn all_layers_bucket_keeps_locked_alpha_for_colour_and_pattern_fills() {
    use photocraft_geom::Rect;

    for depth in [8, 16, 32] {
        for pattern in [false, true] {
            let mut s = Session::new();
            s.execute("file.new", json!({"width": 20, "height": 10, "depth": depth, "background": "white"})).unwrap();
            let background = s.active().unwrap().active_layer.unwrap();
            s.edit("wall", |doc, _| {
                doc.layer_mut(background).unwrap().surface_mut().unwrap().fill_rect(Rect::new(10, 0, 11, 10), &[0.0, 0.0, 0.0, 1.0]);
                Ok(())
            })
            .unwrap();
            s.execute("layer.new.layer", json!({})).unwrap();
            s.edit("alpha regions", |doc, active| {
                let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
                for x in [2, 12] {
                    surf.fill_rect(Rect::new(x, 0, x + 6, 10), &[1.0, 1.0, 1.0, 0.5]);
                    surf.fill_rect(Rect::new(x + 4, 0, x + 6, 10), &[1.0, 1.0, 1.0, 1.0]);
                }
                Ok(())
            })
            .unwrap();
            s.execute("layer.lockLayers", json!({"transparency": true})).unwrap();
            s.execute("select.rect", json!({"x": 0, "y": 2, "width": 20, "height": 6})).unwrap();
            let before = s.active().unwrap().doc.clone();
            let active = s.active().unwrap().active_layer.unwrap();
            let old = before.layer(active).unwrap().surface().unwrap();
            let mut params = json!({"x": 2, "y": 5, "color": "#0000ff", "antiAlias": false, "tolerance": 0,
                "opacity": 50, "sampleAllLayers": true});
            if pattern {
                params["contents"] = json!("pattern");
                params["pattern"] = json!("Diagonal Lines");
            }
            assert_eq!(s.execute("paint.bucket", params).unwrap()["filled"], true);
            let after = s.active().unwrap().doc.clone();
            let new = after.layer(active).unwrap().surface().unwrap();
            let mut recoloured = false;
            for y in 0..10 {
                for x in 0..20 {
                    let (a, b) = (old.rgba(x, y), new.rgba(x, y));
                    assert_eq!(b[3], a[3], "depth {depth}, pattern {pattern}, ({x}, {y}): alpha changed");
                    if x >= 10 || !(2..8).contains(&y) {
                        assert_eq!(b, a, "outside the sampled region or selection");
                    } else if a[3] > 0.0 {
                        recoloured |= a[..3] != b[..3];
                    }
                }
            }
            assert!(recoloured, "depth {depth}, pattern {pattern}: locked pixels still receive paint");
            assert_eq!(after.layer(background), before.layer(background), "sampled background is untouched");
            assert_eq!(after.selection, before.selection);
            assert!(s.undo());
            assert_eq!(*s.active().unwrap().doc, *before);
            assert!(s.redo());
            assert_eq!(*s.active().unwrap().doc, *after);
        }
    }
}
