project(
  'geopard',
  'rust',
  version: '1.5.0',
  meson_version: '>= 0.59',
  # license: MIT,
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'com.ranfdev.Geopard'

dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.0.0')

glib_compile_resources = find_program('glib-compile-resources', required: true)
glib_compile_schemas = find_program('glib-compile-schemas', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)

cargo = find_program('cargo', required: true)

version = meson.project_version()

bindir = join_paths(get_option('prefix'), get_option('bindir'))
localedir = join_paths(get_option('prefix'), get_option('localedir'))
pkgdatadir = join_paths(get_option('prefix'), get_option('datadir'), meson.project_name())
iconsdir = join_paths(get_option('prefix'), get_option('datadir'), 'icons')
podir = join_paths(meson.project_source_root(), 'po')

gettext_package = meson.project_name()

if get_option('profile') == 'development'
  profile = 'Devel'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: true).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
  application_id = '@0@.@1@'.format(base_id, profile)
else
  profile = ''
  version_suffix = ''
  application_id = base_id
endif


meson.add_dist_script(
  'build-aux/dist-vendor.sh',
  join_paths(meson.project_build_root(), 'meson-dist', meson.project_name() + '-' + version),
  meson.project_source_root()
)

if get_option('profile') == 'development'
  # Setup pre-commit hook for ensuring coding style is always consistent
  message('Setting up git pre-commit hook..')
  run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit', check: false)
  run_command('chmod', '+x', '.git/hooks/pre-commit', check: false)
endif

subdir('data')
subdir('po')
subdir('src')

if get_option('profile') == 'development'
  devenv = environment()

  devenv.prepend('XDG_DATA_DIRS', [
    join_paths(meson.current_build_dir(), get_option('datadir'))
  ])

  meson.add_devenv(devenv)
endif

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: true,
  update_desktop_database: true,
)

