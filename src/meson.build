# Generate config module
configure_file(
  input: 'build_config.rs.in',
  output: 'build_config.rs',
  configuration: configuration_data({
    'APP_ID': APPLICATION_ID,
    'ROOT_PATH': ROOT_PATH,
    'RELEASE_VER': meson.project_version(),
    'VERSION': meson.project_version() + VERSION_SUFFIX,
    'PROFILE': get_option('profile'),
    'PROJECT_URL': PROJECT_URL,
    'REPO_URL': REPO_URL,
    'BUGTRACKER_URL': BUGTRACKER_URL,
    'DONATIONS_URL': DONATIONS_URL,
    #'TRANSLATE_URL': TRANSLATE_URL,
    'PKGDATA_DIR': PKGDATA_DIR,
    'LOCALE_DIR': conf.get('LOCALE_DIR'),
    'GETTEXT_PACKAGE': meson.project_name()
  })
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

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: BIN_DIR,
  depends: resources,
  command: [
    'env',
    cargo_env,
    cargo_bin, 'build',
    cargo_options,
    '&&',
    'cp', join_paths('src', rust_target, meson.project_name()), '@OUTPUT@',
  ]
)
