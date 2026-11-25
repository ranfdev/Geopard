blueprints = custom_target('blueprints',
  input: files(
    'ui/window.blp',
    'ui/shortcuts.blp',
    'ui/input_page.blp',
    'ui/download_page.blp',
    'ui/tab.blp'
  ),
  output: '.',
  command: [find_program('blueprint-compiler'), 'batch-compile', '@OUTPUT@', '@CURRENT_SOURCE_DIR@', '@INPUT@'],
)

resources = gnome.compile_resources(
  'resources',
  'resources.gresource.xml',
  gresource_bundle: true,
  source_dir: meson.current_build_dir(),
  install: true,
  install_dir: PKGDATA_DIR,
  dependencies: blueprints,
)

