use bytemuck::Zeroable;
use glam::Vec2Swizzles;
use winit::dpi::PhysicalSize;

use crate::shaders::{rect_render_shader, tris_render_shader};

pub enum Mode {
    /// Makes it so that the source view completely fills up the destination view, cutting off
    /// parts of the source as necessary to preserve aspect ratio.
    Cover,
    /// Makes it so that the source view fills up as much of the destination view as it can,
    /// scaling down linearly to preserve aspect ratio.
    Fit,
}

#[derive(Zeroable, Debug)]
pub struct Uniforms {
    pub scale: glam::Vec2,
    pub offset: glam::Vec2,
    pub lower_bound: glam::Vec2,
    pub upper_bound: glam::Vec2,
}

impl From<Uniforms> for tris_render_shader::Uniforms {
    fn from(uniforms: Uniforms) -> Self {
        let Uniforms {
            scale,
            offset,
            lower_bound,
            upper_bound,
        } = uniforms;
        tris_render_shader::Uniforms {
            scale,
            offset,
            lower_bound,
            upper_bound,
        }
    }
}

impl From<Uniforms> for rect_render_shader::Uniforms {
    fn from(uniforms: Uniforms) -> Self {
        let Uniforms {
            scale,
            offset,
            lower_bound,
            upper_bound,
        } = uniforms;
        rect_render_shader::Uniforms {
            scale,
            offset,
            lower_bound,
            upper_bound,
        }
    }
}

pub struct ScreenRect {
    pub width: f32,
    pub height: f32,
}

impl From<PhysicalSize<u32>> for ScreenRect {
    fn from(size: PhysicalSize<u32>) -> Self {
        Self {
            width: size.width as f32,
            height: size.height as f32,
        }
    }
}

pub struct SourceRect {
    pub width: f32,
    pub height: f32,
}

pub struct DestinationRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Uniforms {
    /// Takes a "source" rectangle (just width/height) and returns a set of parameters that will
    /// blit it onto the screen at a "destination" rectangle (x/y/width/height).
    /// It assumes the output parameters will be used inside a vertex shader like:
    /// ```wgsl
    /// @group(0) @binding(0) var<uniform> uni: Uniforms;
    /// @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    ///     // Calculate xy based on input geometry
    ///     return vec4f(xy * uni.scale + uni.offset, 0.0, 1.0);
    /// }
    /// ```
    /// and inside the fragment shader like:
    /// ```wgsl
    /// @group(0) @binding(0) var<uniform> uni: Uniforms;
    /// @fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    ///     let xy = p.xy;
    ///     if (all(uni.lower_bound <= xy) && all(xy <= uni.upper_bound)) {
    ///         // Calculate & return color
    ///     }
    ///     discard;
    /// }
    /// ```
    pub fn source_to_screen(
        screen: ScreenRect,
        source: SourceRect,
        destination: DestinationRect,
        mode: Mode,
    ) -> Self {
        if source.width <= 0.0
            || source.height <= 0.0
            || destination.width <= 0.0
            || destination.height <= 0.0
        {
            return Uniforms::zeroed();
        }

        /*
         * The overall transformation we want to accomplish is transforming the "source pixels" of
         * the source to the "destination pixels" of the screen, while preserving aspect ratio.
         * This transformation can be modeled as follows:
         *
         * $$
         * t: pxs -> pxd
         * t(pxs) = pxs * (s, s) + (o_x, o_y)
         * $$
         *
         * When preserving aspect ratio, there are two things we can do: "fit" or "cover". Both look
         * at both possible scaling ratios, $w_d / w_s$ and $h_d / h_s$, where "fit" takes the
         * minimum and "cover" takes the maximum. Here, we decide to use "cover", though all
         * following equations will work with either:
         *
         * $$
         * s = max(d_w / s_w, d_h / s_h)
         * $$
         *
         * Then, we need to set a boundary condition to find the correct offset. In our case, we'd
         * like to center the image, which can be expressed as:
         *
         * $$
         * t(s_w/2, s_h/2) = (x + d_w/2, u + d_h/2)
         * $$
         *
         * And, solving:
         *
         * $$
         * => s * s_w/2 + o_x = x + d_w/2, s * s_h / 2 + o_y = y + d_h/2
         * => o_x = x + 0.5*d_w - s*0.5*s_w, o_y = y + 0.5*d_h - s*0.5*s_h
         * $$
         */
        let source_size = glam::vec2(source.width, source.height);
        let destination_size = glam::vec2(destination.width, destination.height);
        let destination_offset = glam::vec2(destination.x, destination.y);
        let direct_scale = destination_size / source_size;
        let overall_scale = match mode {
            Mode::Cover => {
                // Take maximum
                if direct_scale.x > direct_scale.y {
                    direct_scale.xx()
                } else {
                    direct_scale.yy()
                }
            }
            Mode::Fit => {
                // Take minimum
                if direct_scale.x < direct_scale.y {
                    direct_scale.xx()
                } else {
                    direct_scale.yy()
                }
            }
        };
        let overall_offset =
            destination_offset + 0.5 * (destination_size - overall_scale * source_size);

        /*
         * However! There is another transformation we have to account for: the automatic
         * translation from the vertex shader position in clip space to the render shader position
         * in screen space. This looks something like:
         *
         * -1      0      1         0            r_w
         *  . ---- . ---- . 1       . ---- . ---- . 0
         *  |      |      |         |      |      |
         *  |      |      |         |      |      |
         *  . ---- . ---- . 0   =>  . ---- . ---- .
         *  |      |      |         |      |      |
         *  |      |      |         |      |      |
         *  . ---- . ---- . -1      . ---- . ---- . r_h
         *
         *
         * $$
         * uv_to_pxd: uv -> pxd
         * uv_to_pxd(uv) => uv * (r_w/2, -r_h/2) + (r_w/2, r_h/2)
         * $$
         *
         * However, we only have control over the following transformation:
         *
         * $$
         * pxs_to_uv: pxs -> uv
         * pxs_to_uv(pxs) = ??
         * $$
         *
         * So, we want to satisfy the following equation, solving for the $scale$ and $offset$
         * vectors that make up $pxs_to_uv$:
         *
         * $$
         * t(pxs) = uv_to_pxd(pxs_to_uv(pxs))
         * $$
         *
         * It's possible to analyze that equation, but it's a bit tedious. Instead, let's model
         * each transformation with homogenous coordinates, so it just becomes a series of matrix
         * multiplications:
         *
         * $$
         *    T * pxs = uv_to_pxd * pxs_to_uv * pxs
         * => T = uv_to_pxd * pxs_to_uv
         * => uv_to_pxd^{-1} * T = pxs_to_uv
         * => pxs_to_uv = [[ r_w/2,      0, r_w/2 ],
         *                 [     0, -r_h/2, r_h/2 ],
         *                 [     0,      0,     1 ]]^{-1}
         *               * [[ s, 0, o_x ],
         *                  [ 0, s, o_y ],
         *                  [ 0, 0,   1 ]]
         * => pxs_to_uv = [[ 2/r_w,      0, -1 ],
         *                 [     0, -2/r_h,  1 ],
         *                 [     0,      0,  1 ]]
         *               * [[ s, 0, o_x ],
         *                  [ 0, s, o_y ],
         *                  [ 0, 0,   1 ]]
         * => pxs_to_uv = [[ 1,  0, 0 ],
         *                 [ 0, -1, 0 ],
         *                 [ 0,  0, 1 ]]
         *              * [[ 2*s/r_w,       0, 2*o_x/r_w - 1 ],
         *                 [       0, 2*s/r_h, 2*o_y/r_h - 1 ],
         *                 [       0,       0,             1 ]]
         * $$
         *
         * For convenience, we've factored out the y-flip at the end.
         */
        let screen_size = glam::vec2(screen.width, screen.height);
        let scale = 2.0 * overall_scale / screen_size;
        let offset = 2.0 * overall_offset / screen_size - 1.0;
        let flip = glam::vec2(1.0, -1.0);
        let scale = scale * flip;
        let offset = offset * flip;

        /*
         * Because we are using a "cover" transform, we need to clip the edges of the texture to the
         * exact places we're drawing to on the screen. Specifically, everything between (x, y)pxd
         * and (x + width, y + height)pxd is allowed to be drawn, and anything outside needs to be
         * set transparent.
         *
         * Fortunately, these coordinates the fragment shader works on are already framebuffer
         * coordinates, so we can just use those directly:
         */
        let lower_bound = destination_offset;
        let upper_bound = destination_offset + destination_size;

        Uniforms {
            scale,
            offset,
            lower_bound,
            upper_bound,
        }
    }
}
