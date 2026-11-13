#!/usr/bin/env bash
set -euo pipefail

invocation_dir=$PWD
cd "$(dirname "$0")"

usage() {
  cat <<'EOF'
Usage: ./h5jive.sh [-b|--build] [-r|--run] [--view-VIEW] [--backtrace|--no-backtrace] [-c|--clean] [FILE_A[:/DATASET] [FILE_B[:/DATASET]]]
       ./h5jive.sh --website

  -b, --build   Build the release viewer; run it if file paths or --run are supplied
  -r, --run     Run the existing build; use the bundled demo if no files are supplied
  --backtrace   Show a stack trace if the viewer panics
  --no-backtrace  Disable stack traces for this run
  --view-VIEW   Open the selected dataset in a specific view (requires :/DATASET)
                VIEW: data, 1d-profiles, 2d-slice, 3d, world-map, globe
  -c, --clean   Remove Cargo build output and exit
  --website     Start the local website preview server (Ctrl+C to stop)
  -h, --help    Show this help

With no options or file paths, this help is shown. Supply one or two HDF5 or
NetCDF files with --run (or directly) to open them. Add :/path/to/dataset to
select a dataset at startup. Paths containing spaces are supported.
EOF
}

build=false
run=false
clean=false
website=false
backtrace_override=
view_override=
files=()
while (($#)); do
  case "$1" in
    -b|--build) build=true ;;
    -r|--run) run=true ;;
    -c|--clean) clean=true ;;
    --website) website=true ;;
    --backtrace) backtrace_override=1 ;;
    --no-backtrace) backtrace_override=0 ;;
    --view-data|--view-1d-profiles|--view-2d-slice|--view-3d|--view-world-map|--view-globe)
      if [[ -n $view_override ]]; then
        echo "Specify only one --view-* option." >&2
        exit 2
      fi
      view_override=$1 ;;
    -h|--help) usage; exit 0 ;;
    --) shift; files+=("$@"); break ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    *) files+=("$1") ;;
  esac
  shift
done

if "$website"; then
  if "$build" || "$run" || "$clean" || [[ -n $backtrace_override || -n $view_override ]] || ((${#files[@]} > 0)); then
    echo "--website cannot be combined with viewer options or input files." >&2
    exit 2
  fi
  if ! command -v npm >/dev/null 2>&1; then
    echo "npm is required to run the website preview." >&2
    exit 1
  fi
  if [[ ! -d website/node_modules ]]; then
    echo "Website dependencies are missing. Run: cd website && npm install" >&2
    exit 1
  fi
  cd website
  exec npm run dev -- --host 127.0.0.1
fi

if ((${#files[@]} > 2)); then
  echo "H5Jive accepts at most two input files." >&2
  exit 2
fi

if "$clean"; then
  cargo clean
  exit 0
fi

if ! "$build" && ! "$run" && ((${#files[@]} == 0)) && [[ -z $view_override ]]; then
  usage
  exit 0
fi

if "$run" && ((${#files[@]} == 0)) && [[ -z $view_override ]]; then
  files=("$PWD/examples/space_demo.hdf5")
fi

dataset_selected=false
for i in "${!files[@]}"; do
  input=${files[$i]}
  file=$input
  suffix=
  if [[ $input == *:/* ]]; then
    file=${input%:/*}
    suffix=${input:${#file}}
    if [[ $file == [A-Za-z] ]]; then
      file=$input
      suffix=
    fi
  fi
  if [[ $file != /* ]]; then file="$invocation_dir/$file"; fi
  if [[ ! -f $file ]]; then
    echo "Input file does not exist: $file" >&2
    exit 2
  fi
  files[$i]="$file$suffix"
  if [[ -n $suffix ]]; then dataset_selected=true; fi
done

if [[ -n $view_override ]] && ! "$dataset_selected"; then
  echo "A --view-* option requires FILE:/DATASET." >&2
  exit 2
fi

if "$build"; then
  cargo build --release -p h5jive
fi

if ((${#files[@]} == 0)); then
  exit 0
fi

if [[ ! -x target/release/h5jive ]]; then
  echo "H5Jive is not built. Run ./h5jive.sh --build first." >&2
  exit 1
fi

if [[ -n $backtrace_override ]]; then
  export RUST_BACKTRACE=$backtrace_override
fi

if [[ -n $view_override ]]; then
  exec target/release/h5jive "$view_override" "${files[@]}"
fi
exec target/release/h5jive "${files[@]}"

