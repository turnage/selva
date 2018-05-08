# selva

selva is my glsl live coding daemon. It supports noninteractive ShaderToy shaders.

Install it with `cargo install selva`.

## Ui

````
selva 0.1.0
A glsl live coding daemon.

USAGE:
    selva [OPTIONS] <FRAG>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f, --frames <frames>              Frame range to render from the generate scene. [default: 1]
    -h, --height <height>              Height of view pane. [default: 500]
    -I, --include <include_dirs>...    Include directories.
    -o, --output <output>              Output directory or filename.
    -w, --width <width>                Width of view pane. [default: 500]

ARGS:
    <FRAG>    Fragment shader to run.
````


## Features

* Live GLSL source refresh with `#include` support. The whole tree is watched.
* Render out to file using GPU at any size; selva will render tile-wise so you can far exceed device memory. Render 4k! Render 8k! Render 24k!
