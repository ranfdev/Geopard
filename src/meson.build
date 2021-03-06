global_conf = configuration_data()
global_conf.set_quoted('APP_ID', application_id)
global_conf.set_quoted('PKGDATADIR', pkgdatadir)
global_conf.set_quoted('PROFILE', profile)
global_conf.set_quoted('NAME_SUFFIX', name_suffix)
global_conf.set_quoted('VERSION', version + version_suffix)
global_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
global_conf.set_quoted('LOCALEDIR', localedir)

build_config_rs = configure_file(
  input: 'build_config.rs.in',
  output: 'build_config.rs',
  configuration: global_conf
)

run_command(
  'cp',
  build_config_rs,
  meson.current_source_dir(),
  check: true
)

sources = files(
  'common/mod.rs',
  'component.rs',
  'tab.rs',
  'config.rs',
  'window.rs',
  'gemini/mod.rs',
  'gemini/client.rs',
  'gemini/parser.rs',
  'config.rs',
  'main.rs',
)

custom_target(
  'cargo-build',
  build_by_default: true,
  input: sources,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  command: [
    cargo_script,
    meson.build_root(),
    meson.source_root(),
    '@OUTPUT@',
    profile,
    meson.project_name(),
  ]
)
