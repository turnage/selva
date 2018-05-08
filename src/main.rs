#[macro_use]
extern crate glium;
extern crate glsl_include;
extern crate glslwatch;
#[macro_use]
extern crate structopt;
extern crate image;
extern crate itertools;

use structopt::StructOpt;
use glium::{glutin, Display};
use glium::backend::glutin::DisplayCreationError;
use glium::program::Program;
use glslwatch::GLSLTree;
use std::{thread, time};
use glium::Surface;
use glium::{IndexBuffer, VertexBuffer};
use glium::index::PrimitiveType;
use std::time::{Duration, Instant};
use glium::glutin::HeadlessRendererBuilder;
use glium::backend::glutin::headless::Headless;
use glium::texture::texture2d::Texture2d;
use image::ImageBuffer;
use image::Rgba;
use itertools::Itertools;

const TILE_SIZE: u32 = 1080;

const VERTEX_SHADER: &str = r#"
#version 150

in vec2 vpos;

void main() {
    gl_Position = vec4(vpos, 0.0, 0.0);
}
"#;

const UNIFORM_DECLS: &str = r#"
uniform vec3 iResolution;
uniform float iGlobalTime;
uniform float iTime;
uniform int iFrame;
uniform float iLinearize;
uniform vec2 iTileResolution;
uniform vec2 iTileIndex;
"#;

const SHADERTOY_MAIN: &str = r#"
out vec4 _selva_color;

void main() {
    mainImage(_selva_color, gl_FragCoord.xy  + iTileResolution.xy * iTileIndex);
    bvec4 cutoff = lessThan(_selva_color, vec4(0.04045));
    vec4 higher = pow((_selva_color + vec4(0.055))/vec4(1.055), vec4(2.4));
    vec4 lower = _selva_color/vec4(12.92);

    vec4 iLinearized_color = mix(higher, lower, cutoff);
    _selva_color = _selva_color * (1.0 - iLinearize)
                 + iLinearize * iLinearized_color;
}
"#;

#[derive(Debug, Copy, Clone)]
pub struct GpuVertex {
    pub vpos: [f32; 2],
}

implement_vertex!(GpuVertex, vpos);

const TWO_TRIANGLES: (&[GpuVertex], &[u8]) = (
    &[
        GpuVertex { vpos: [-1.0, -1.0] },
        GpuVertex { vpos: [-1.0, 1.0] },
        GpuVertex { vpos: [1.0, 1.0] },
        GpuVertex { vpos: [1.0, -1.0] },
    ],
    &[0, 1, 2, 2, 3, 0],
);

#[derive(StructOpt, Debug)]
#[structopt(name = "selva")]
struct Options {
    /// Frame range to render from the generate scene.
    #[structopt(short = "f", long = "frames", default_value = "1")]
    frames: usize,

    /// Width of view pane.
    #[structopt(short = "w", long = "width", default_value = "500")]
    width: u32,

    /// Height of view pane.
    #[structopt(short = "h", long = "height", default_value = "500")]
    height: u32,

    /// Include directories.
    #[structopt(short = "I", long = "include")]
    include_dirs: Vec<String>,

    /// Fragment shader to run.
    #[structopt(name = "FRAG")]
    frag: String,

    /// Output directory or filename.
    #[structopt(short = "o", long = "output")]
    output: Option<String>
}

fn window(width: u32, height: u32) -> Result<(Display, glutin::EventsLoop), DisplayCreationError> {
    let events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("Valora".to_string())
        .with_dimensions(width, height);
    let context = glutin::ContextBuilder::new()
        .with_multisampling(16)
        .with_pixel_format(24, 8)
        .with_srgb(false)
        .with_vsync(true);
    let display = Display::new(window, context, &events_loop)?;

    Ok((display, events_loop))
}

fn format_time(dur: Duration) -> f32 {
    dur.as_secs() as f32 + dur.subsec_nanos() as f32 / 1.0e9
}

fn prepend_uniforms(src: &str) -> String {
    let mut lines = src.lines();
    let version = lines.next().unwrap();
    let prefixes = vec![version, UNIFORM_DECLS];
    let suffixes = if src.contains("void mainImage(") {
        vec![SHADERTOY_MAIN]
    } else {
        Vec::new()
    };
    prefixes
        .into_iter()
        .chain(lines)
        .chain(suffixes.into_iter())
        .collect::<Vec<&str>>()
        .join("\n")
}

fn display(options: Options) {
    let (window, _) = window(options.width, options.height).expect("window");
    let vertices = VertexBuffer::new(&window, TWO_TRIANGLES.0).expect("vertex buffer");
    let indices = IndexBuffer::new(&window, PrimitiveType::TrianglesList, TWO_TRIANGLES.1)
        .expect("index buffer");
    let mut prog = None;
    let mut src_r = GLSLTree::new(&options.frag, &options.include_dirs);
    let mut start_time = Instant::now();
    let mut frame_num = 0;
    let mut src_str;
    loop {
        let (src_r_, prog_) = match (
            src_r.and_then(|src| match src.expired()? {
                true => Ok((src.refresh()?, true)), // Should make new program.
                false => Ok((src, false)),          // Should not make new progam.
            }),
            prog,
        ) {
            (Err(e), prog) => {
                println!("Load error: {:?}", e);
                (GLSLTree::new(&options.frag, &options.include_dirs), prog)
            }
            (Ok((src, true)), prog) => {
                src_str = prepend_uniforms(src.render());
                match Program::from_source(&window, VERTEX_SHADER, &src_str, None) {
                    Err(e) => {
                        println!("Compile error: {:?}", e);
                        (Ok(src), prog)
                    }
                    Ok(p) => {
                        start_time = Instant::now();
                        frame_num = 0;
                        (Ok(src), Some(p))
                    }
                }
            }
            (Ok((src, false)), None) => {
                src_str = prepend_uniforms(src.render());
                match Program::from_source(&window, VERTEX_SHADER, &src_str, None) {
                    Err(e) => {
                        println!("Compile error: {:?}", e);
                        (Ok(src), None)
                    }
                    Ok(p) => {
                        start_time = Instant::now();
                        frame_num = 0;
                        (Ok(src), Some(p))
                    }
                }
            }
            (Ok((src, _)), prog) => (Ok(src), prog),
        };
        src_r = src_r_;
        prog = prog_;
        if let Some(ref p) = prog {
            let elapsed = format_time(start_time.elapsed());
            let mut frame = window.draw();
            frame
                .draw(
                    &vertices,
                    &indices,
                    p,
                    &uniform!{
                        iResolution: (
                            options.width as f32,
                            options.height as f32,
                            (options.width as f32) / (options.height as f32)
                        ),
                        iGlobalTime: elapsed,
                        iTime: elapsed,
                        iFrame: frame_num,
                        iLinearize: 1.0 as f32,
                    },
                    &Default::default(),
                )
                .expect("draw");
            frame.finish().expect("buffer switch");
            frame_num += 1;
        }
        thread::sleep(time::Duration::from_millis(16));
    }
}

fn to_file(output: String, options: Options) {
    let tiler = |dim: u32| (0..).map(|i| dim / std::cmp::max(i * 2, 1)).enumerate().map(|(i, d)| (std::cmp::max(i * 2, 1), d)).find(|&(_, ref d)| *d < TILE_SIZE).unwrap();
    let (tiles_wide, tile_width) = tiler(options.width);
    let (tiles_high, tile_height) = tiler(options.height);
    let (renderer, _) = window(tile_width, tile_height).expect("window");
    let vertices = VertexBuffer::new(&renderer, TWO_TRIANGLES.0).expect("vertex buffer");
    let indices = IndexBuffer::new(&renderer, PrimitiveType::TrianglesList, TWO_TRIANGLES.1)
        .expect("index buffer");
    let buffer = Texture2d::empty(&renderer, tile_width, tile_height).expect("texture buffer");
    let program_src = prepend_uniforms(GLSLTree::new(options.frag, &options.include_dirs).expect("source tree").render());
    let program = Program::from_source(&renderer, VERTEX_SHADER, &program_src, None).expect("compiling shader");

    println!("{}x{} tiles; {}x{} tile dims", tiles_wide, tiles_high, tile_width, tile_height);

    for frame in 0..(options.frames) {
        let mut frame_buffer: Vec<u8> = Vec::new();
        for i in 0..tiles_high {
            let mut row_buffer = Vec::new();
            for j in 0..tiles_wide {
                buffer.as_surface().draw(
                    &vertices,
                    &indices,
                    &program,
                    &uniform!{
                        iResolution: (
                            options.width as f32,
                            options.height as f32,
                            (options.width as f32) / (options.height as f32)
                        ),
                        iGlobalTime: frame as f32 * 0.016,
                        iTime: frame as f32 * 0.016,
                        iFrame: frame as u32,
                        iLinearize: 0.0 as f32,
                        iTileResolution: (tile_width as f32, tile_height as f32),
                        iTileIndex: (j as f32, i as f32)
                    },
                    &Default::default()).expect("draw to texture");
                let raw: glium::texture::RawImage2d<u8> = buffer.read();
                row_buffer.push(raw.data.into_owned());
            }

            for k in 0..tile_height {
                for j in 0..tiles_wide {
                    frame_buffer.extend(row_buffer[j][(k as usize * tile_width as usize * 4)..((k + 1) as usize * tile_width as usize * 4)].iter());
                }
            }
        }

        println!("{:?} bytes in framebuffer", frame_buffer.len());
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(options.width, options.height, frame_buffer).unwrap();
        img.save(format!("{}_{}.png", output, frame)).expect("saved image");

    }
}

fn main() {
    let options = Options::from_args();
    if options.output.is_some() {
        to_file(options.output.as_ref().unwrap().clone(), options)
    } else {
        display(options)
    }
}
