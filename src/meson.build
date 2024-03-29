global_conf = configuration_data()
global_conf.set_quoted('APP_ID', application_id)
global_conf.set_quoted('PKGDATADIR', pkgdatadir)
global_conf.set_quoted('PROFILE', profile)
global_conf.set_quoted('VERSION', version + version_suffix)
global_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
global_conf.set_quoted('LOCALEDIR', localedir)

config = configure_file(
  input: 'build_config.rs.in',
  output: 'build_config.rs',
  configuration: global_conf
)

# Copy the config.rs output to the source directory.
run_command(
  'cp',
  join_paths(meson.project_build_root(), 'src', 'build_config.rs'),
  join_paths(meson.project_source_root(), 'src', 'build_config.rs'),
  check: true
)

cargo_options = [ '--manifest-path', join_paths(meson.project_source_root(), 'Cargo.toml') ]
cargo_options += [ '--target-dir', join_paths(meson.project_build_root(), 'src') ]

if get_option('profile') == 'default'
  cargo_options += [ '--release' ]
  rust_target = 'release'
  message('Building in release mode')
else
  rust_target = 'debug'
  message('Building in debug mode')
endif

if get_option('offline')
  cargo_options += [ '--offline' ]
  message('Building offline')
endif

cargo_env = [ 'CARGO_HOME=' + join_paths(meson.project_build_root(), 'cargo-home') ]

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  command: [
    'env',
    cargo_env,
    cargo, 'build',
    cargo_options,
    '&&',
    'cp', join_paths('src', rust_target, meson.project_name()), '@OUTPUT@',
  ]
)
