#!/usr/bin/env bash
set -euo pipefail

invocation_dir=$PWD
cd "$(dirname "$0")"

usage() {
  cat <<'EOF'
Usage: ./h5jive.sh [-b|--build] [-r|--run] [-c|--clean] [FILE_A[:/DATASET] [FILE_B[:/DATASET]]]

  -b, --build   Build the release viewer; run it if file paths or --run are supplied
  -r, --run     Run the existing build; use the bundled demo if no files are supplied
  -c, --clean   Remove Cargo build output and exit
  -h, --help    Show this help

With no options or file paths, this help is shown. Supply one or two HDF5 or
NetCDF files with --run (or directly) to open them. Add :/path/to/dataset to
select a dataset at startup. Paths containing spaces are supported.
EOF
}

build=false
run=false
clean=false
files=()
while (($#)); do
  case "$1" in
    -b|--build) build=true ;;
    -r|--run) run=true ;;
    -c|--clean) clean=true ;;
    -h|--help) usage; exit 0 ;;
    --) shift; files+=("$@"); break ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    *) files+=("$1") ;;
  esac
  shift
done

if ((${#files[@]} > 2)); then
  echo "H5Jive accepts at most two input files." >&2
  exit 2
fi

if "$clean"; then
  cargo clean
  exit 0
fi

if ! "$build" && ! "$run" && ((${#files[@]} == 0)); then
  usage
  exit 0
fi

if "$run" && ((${#files[@]} == 0)); then
  files=("$PWD/examples/space_demo.hdf5")
fi

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
done

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

exec target/release/h5jive "${files[@]}"

