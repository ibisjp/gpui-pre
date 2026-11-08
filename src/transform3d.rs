// Added by ibis inc. for ibisOffice (2026-11-08): a 4x4 matrix for drawing an offscreen texture in 3D (`Window::paint_textured_quad`).
//
// Points are (x, y, z) in logical pixels of the window: x to the right, y down, and z toward the viewer
// (a point with z > 0 is nearer the eye, so it is drawn larger once a perspective is applied).
// Matrices are column-major (`m[column * 4 + row]`), as GPUs read them, and a point is a column vector,
// so `a.then(b)` (apply `a`, then `b`) is the product `b * a`.

use std::f32::consts::PI;

/// A 4x4 transform of 3D points, for [`Window::paint_textured_quad`](crate::Window::paint_textured_quad).
///
/// Added in ibisOffice's fork of gpui-pre 0.3.6.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3D {
    /// Column-major: element `m[column * 4 + row]`.
    pub m: [f32; 16],
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform3D {
    /// The transform that moves nothing.
    pub const fn identity() -> Self {
        Self {
            m: [
                1., 0., 0., 0., //
                0., 1., 0., 0., //
                0., 0., 1., 0., //
                0., 0., 0., 1.,
            ],
        }
    }

    /// A move by (x, y, z).
    pub const fn translate(x: f32, y: f32, z: f32) -> Self {
        Self {
            m: [
                1., 0., 0., 0., //
                0., 1., 0., 0., //
                0., 0., 1., 0., //
                x, y, z, 1.,
            ],
        }
    }

    /// A scale about the origin.
    pub const fn scale(x: f32, y: f32, z: f32) -> Self {
        Self {
            m: [
                x, 0., 0., 0., //
                0., y, 0., 0., //
                0., 0., z, 0., //
                0., 0., 0., 1.,
            ],
        }
    }

    /// A turn about the x axis (a horizontal line). At a positive angle the bottom edge (+y) comes toward the viewer.
    pub fn rotate_x(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self {
            m: [
                1., 0., 0., 0., //
                0., c, s, 0., //
                0., -s, c, 0., //
                0., 0., 0., 1.,
            ],
        }
    }

    /// A turn about the y axis (a vertical line). At a positive angle the right edge (+x) goes away from the viewer.
    pub fn rotate_y(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self {
            m: [
                c, 0., -s, 0., //
                0., 1., 0., 0., //
                s, 0., c, 0., //
                0., 0., 0., 1.,
            ],
        }
    }

    /// A turn about the z axis (the line toward the viewer). At a positive angle the turn is clockwise on the screen.
    pub fn rotate_z(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self {
            m: [
                c, s, 0., 0., //
                -s, c, 0., 0., //
                0., 0., 1., 0., //
                0., 0., 0., 1.,
            ],
        }
    }

    /// The same as [`Transform3D::rotate_y`], in degrees.
    pub fn rotate_y_degrees(degrees: f32) -> Self {
        Self::rotate_y(degrees * PI / 180.)
    }

    /// A perspective for an eye on the z axis, `distance` in front of the z = 0 plane (looking at it), as the camera of a
    /// pinhole. A point at depth z is drawn about the origin scaled by `distance / (distance - z)`; the plane z = 0 is
    /// not scaled. A point at z >= `distance` is behind the eye and is cut away.
    pub const fn perspective(distance: f32) -> Self {
        Self {
            m: [
                1., 0., 0., 0., //
                0., 1., 0., 0., //
                0., 0., 1., -1. / distance, //
                0., 0., 0., 1.,
            ],
        }
    }

    /// `self` with its origin moved to (x, y): the transform `T(x, y) * self * T(-x, -y)`, so a turn or a perspective
    /// is about that point of the plane (the slide's middle) instead of the window's top left.
    pub fn about(self, x: f32, y: f32) -> Self {
        Self::translate(-x, -y, 0.).then(self).then(Self::translate(x, y, 0.))
    }

    /// The transform that applies `self` first and `next` after it (the product `next * self`).
    pub fn then(self, next: Self) -> Self {
        next.mul(self)
    }

    /// The matrix product `self * rhs` (apply `rhs` first).
    pub fn mul(self, rhs: Self) -> Self {
        let mut out = [0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.;
                for k in 0..4 {
                    sum += self.m[k * 4 + row] * rhs.m[col * 4 + k];
                }
                out[col * 4 + row] = sum;
            }
        }
        Self { m: out }
    }

    /// The homogeneous result (x, y, z, w) of the point (x, y, z). The screen position is (x / w, y / w).
    pub fn apply_homogeneous(&self, x: f32, y: f32, z: f32) -> [f32; 4] {
        let m = &self.m;
        let mut out = [0f32; 4];
        for (row, o) in out.iter_mut().enumerate() {
            *o = m[row] * x + m[4 + row] * y + m[8 + row] * z + m[12 + row];
        }
        out
    }

    /// The screen position of the point (x, y, z), or `None` when it is at or behind the eye (w <= 0).
    pub fn project(&self, x: f32, y: f32, z: f32) -> Option<(f32, f32)> {
        let [px, py, _, w] = self.apply_homogeneous(x, y, z);
        (w > 1e-4).then(|| (px / w, py / w))
    }

    /// The same transform for a window drawn at `scale` device pixels per logical pixel: `S * self * S^-1` for the scale
    /// of x and y (z, and so the perspective distance, stays in logical pixels).
    pub(crate) fn to_device(self, scale: f32) -> Self {
        let into_logical = Self::scale(1. / scale, 1. / scale, 1. / scale);
        let out_of_logical = Self::scale(scale, scale, 1.);
        into_logical.then(self).then(out_of_logical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn identity_and_translate() {
        assert!(near(Transform3D::identity().project(3., 4., 0.).unwrap(), (3., 4.)));
        assert!(near(Transform3D::translate(5., -2., 0.).project(3., 4., 0.).unwrap(), (8., 2.)));
    }

    #[test]
    fn perspective_scales_near_points_up_about_the_origin() {
        let p = Transform3D::perspective(1000.);
        // z = 0 is unchanged; a point 500 toward the eye is twice as far from the origin.
        assert!(near(p.project(100., 50., 0.).unwrap(), (100., 50.)));
        assert!(near(p.project(100., 50., 500.).unwrap(), (200., 100.)));
        // Behind the eye is cut.
        assert!(p.project(100., 50., 1000.).is_none());
    }

    #[test]
    fn rotate_y_a_quarter_turn_puts_the_right_edge_away() {
        let r = Transform3D::rotate_y_degrees(90.);
        let [x, _, z, _] = r.apply_homogeneous(10., 0., 0.);
        assert!(x.abs() < 1e-4 && (z + 10.).abs() < 1e-4);
    }

    #[test]
    fn about_keeps_the_centre_fixed() {
        let t = Transform3D::rotate_y_degrees(60.).then(Transform3D::perspective(800.)).about(400., 300.);
        // Build "about" by hand: the centre of the slide stays where it is.
        assert!(near(t.project(400., 300., 0.).unwrap(), (400., 300.)));
        // The right edge goes away (smaller about the centre) and the left comes near.
        let right = t.project(800., 300., 0.).unwrap().0;
        let left = t.project(0., 300., 0.).unwrap().0;
        // The near (left) half is longer on the screen than the far (right) half.
        assert!(right - 400. < 400. - left);
    }

    #[test]
    fn device_scale_matches_scaled_points() {
        let t = Transform3D::rotate_y_degrees(40.).then(Transform3D::perspective(900.)).about(100., 50.);
        let d = t.to_device(2.);
        let a = t.project(30., 20., 0.).unwrap();
        let b = d.project(60., 40., 0.).unwrap();
        assert!(near((a.0 * 2., a.1 * 2.), b));
    }
}
