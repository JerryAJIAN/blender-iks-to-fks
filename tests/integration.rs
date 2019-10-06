extern crate tempfile;

use std::path::PathBuf;
use std::process::Command;
use tempfile::NamedTempFile;

/// Our test blender file has 3 objects - a camera, mesh and armature
/// Here we ensure that after we run the script there are 5 objects,
/// since our script generates a new mesh and armature
#[test]
fn creates_seconds_armature() {
    let output = Command::new("blender")
        .arg(leg_blend())
        .arg("--background")
        .args(&["--python", run_addon_py().to_str().unwrap()])
        .args(&["--python", print_num_objects_py().to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("The number of objects is: 5"),
        "Stdout: {}",
        stdout
    )
}

/// We render a frame of our mesh's animation from before and then after we've run our FK generation script
/// We then compare these two frames and make sure that they are the same. If they
/// are then we know that our second mesh is in the same position as our first mesh at the keyframe that we rendered.
/// Which means that the two meshes share the same animation.
#[test]
fn before_after_test_case_leg() {
    BeforeAfterTestCase {
        blend_file: leg_blend(),
        frame_to_render: 10,
        max_error: 0.0051,
    }
    .test();
}

#[test]
fn before_after_test_case_bezier() {
    BeforeAfterTestCase {
        blend_file: bezier_curve_bone_hooks_deform_off_blend(),
        frame_to_render: 19,
        max_error: 0.0179,
    }
    .test();
}

/// If you have not selected a mesh that has an armature we will use the first mesh that we find that has an armature
/// This makes everything work right out of the box for blender files that only have one armature and mesh.
#[test]
fn automatic_selection() {
    let output = Command::new("blender")
        .arg(unselected_blend())
        .arg("--background")
        .args(&["--python", run_addon_py().to_str().unwrap()])
        .args(&["--python", print_num_objects_py().to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        // Original mesh and armature, new mesh and armature, camera = 5 objects total
        stdout.contains("The number of objects is: 5"),
        "Stdout: {}",
        stdout
    )
}

/// Make sure that all of the duplicate actions that get created when we duplicate our armature
/// and mesh end up getting removed.
#[test]
fn no_new_actions_created() {
    let output = Command::new("blender")
        .arg(leg_blend())
        .arg("--background")
        .args(&["--python", run_addon_py().to_str().unwrap()])
        .args(&["--python", print_num_actions_py().to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("The number of actions is: 1"),
        "Stdout: {}",
        stdout
    )
}

/// Make sure that if an armature has multiple child meshes we duplicate all of them so that all
/// of their vertex groups are set to the new FK armature
#[test]
fn armature_with_multiple_child_meshes() {
    let output = Command::new("blender")
        .arg(multiple_meshes_for_armature())
        .arg("--background")
        .args(&["--python", run_addon_py().to_str().unwrap()])
        .args(&["--python", print_num_objects_py().to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        // Original 2 meshes, original armature, new 2 mesh and new armature, camera = 7 objects total
        stdout.contains("The number of objects is: 7"),
        "Stdout: {}",
        stdout
    )
}

/// Used to verify that before and after running blender-iks-to-fks generates approximately the
/// same animation (just without the IK bones).
///
/// We do this by rendering the before and after animations to images and comparing the root mean
/// square error of these two images.
struct BeforeAfterTestCase {
    blend_file: PathBuf,
    frame_to_render: u16,
    max_error: f32,
}

impl BeforeAfterTestCase {
    fn test(self) {
        let before_img = NamedTempFile::new().unwrap();
        let before_img = before_img.path();

        let after_img = NamedTempFile::new().unwrap();
        let after_img = after_img.path();

        // Render before converting to IK
        let mut before = Command::new("blender")
            .arg(&self.blend_file)
            .arg("--background")
            // Rendering a frame in Eevee isn't working headless in Blender 2.80 when
            // you don't have a display as of October 2019 (i.e. in CI)
            .args(&["-E", "CYCLES"])
            .args(&["--python", run_addon_py().to_str().unwrap()])
            .args(&["--render-output", before_img.to_str().unwrap()])
            .args(&[
                "--render-frame",
                format!("{}", self.frame_to_render).as_str(),
            ])
            .args(&["--render-format", "PNG"])
            .arg("-noaudio")
            .spawn()
            .unwrap();

        // Render after converting to IK
        let mut after = Command::new("blender")
            .arg(&self.blend_file)
            .arg("--background")
            // Rendering a frame in Eevee isn't working headless in Blender 2.80 when
            // you don't have a display as of October 2019 (i.e. in CI)
            .args(&["-E", "CYCLES"])
            .args(&["--render-output", after_img.to_str().unwrap()])
            .args(&[
                "--render-frame",
                format!("{}", self.frame_to_render).as_str(),
            ])
            .args(&["--render-format", "PNG"])
            .arg("-noaudio")
            .spawn()
            .unwrap();

        before.wait().unwrap();
        after.wait().unwrap();

        let output = Command::new("compare")
            .arg("-metric")
            .arg("RMSE")
            .arg(&format!(
                "{}00{}.png",
                before_img.to_str().unwrap(),
                self.frame_to_render
            ))
            .arg(&format!(
                "{}00{}.png",
                after_img.to_str().unwrap(),
                self.frame_to_render
            ))
            .arg("/dev/null")
            .output()
            .unwrap();

        // Compare will write the comparison to stderr.
        // It looks like this:
        //  7.31518 (0.000111623)
        // And we grab this
        //   0.000111623
        let stderr = String::from_utf8(output.stderr).unwrap();
        let mut stderr = stderr.split("(");
        stderr.next().unwrap();
        let stderr = stderr.next().unwrap();
        let mut stderr = stderr.split(")");
        let root_mean_square_error = stderr.next().unwrap();

        let root_mean_square_error = root_mean_square_error.parse::<f32>().unwrap();

        assert!(
            root_mean_square_error < self.max_error,
            "Root square mean error between old and new armature {}. {:?}",
            root_mean_square_error,
            &self.blend_file
        );
    }
}

/// /path/to/blender-iks-to-fks/tests
fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// A python script to print out number of objects in the scene to stdout
fn print_num_objects_py() -> PathBuf {
    tests_dir().join("helper-python-scripts/print-num-objects-to-stdout.py")
}

/// A python script to print the number of actions to stdout
fn print_num_actions_py() -> PathBuf {
    tests_dir().join("helper-python-scripts/print-actions-to-stdout.py")
}

/// A leg with IKs
fn leg_blend() -> PathBuf {
    tests_dir().join("leg.blend")
}

/// A file with an unselected mesh and parent armature
fn unselected_blend() -> PathBuf {
    tests_dir().join("unselected.blend")
}

/// We noticed that if there are bone hooks on a bezier curve that is being used to control a spline IK modifier
/// the ik-to-fk process would only work if those bone hooks had `Deform` set to true in Blender.
///
/// However, these aren't actually deformation bones - so this file ensures that we've fixed this and that
/// things work when `Deform` is false.
fn bezier_curve_bone_hooks_deform_off_blend() -> PathBuf {
    tests_dir().join("bone-hooks.blend")
}

/// An armature that has multiple child meshes.
/// Used to ensure that we're generating and using a duplicate of each mesh since our new FK
/// mesh might need to have different bone weights than the original.
fn multiple_meshes_for_armature() -> PathBuf {
    tests_dir().join("multiple-meshes-for-armature.blend")
}

/// Used to run our blender-iks-to-fks addon
fn run_addon_py() -> PathBuf {
    tests_dir().join("../run-addon.py")
}
