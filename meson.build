project('geopard', 'rust',
          version: '1.5.0',
          license: 'GPL-3.0-or-later',
    meson_version: '>= 0.62.0',
  default_options: ['warning_level=2',
                    'werror=false',
                   ],
)

# Import modules
i18n = import('i18n')
gnome = import('gnome')

# Project information
PROJECT_URL = 'https://ranfdev.com/projects/Geopard'
REPO_URL = 'https://github.com/ranfdev/Geopard'
BUGTRACKER_URL = 'https://github.com/ranfdev/Geopard/issues'
DONATIONS_URL = 'https://github.com/sponsors/ranfdev'
#TRANSLATE_URL = 'https://hosted.weblate.org/projects/ranfdev/Geopard'

# Constants
PROJECT_RDNN_NAME = 'com.ranfdev.Geopard'
ROOT_PATH = '/com/ranfdev/Geopard'
BIN_DIR = join_paths(get_option('prefix'), get_option('bindir'))
PKGDATA_DIR = join_paths(get_option('prefix'), get_option('datadir'), meson.project_name())

# Check if cargo and git exist
cargo_bin = find_program('cargo', required: true)
git_bin = find_program('git', required: false)

# Set APPLICATION_ID and VERSION_SUFFIX
if get_option('profile') == 'development'
  if git_bin.found()
    VCS_TAG = run_command('git', 'rev-parse', '--short', 'HEAD', check: true).stdout().strip()
  else
    VCS_TAG = ''
  endif
  if VCS_TAG == ''
    VERSION_SUFFIX = '-devel'
  else
    VERSION_SUFFIX = '-@0@'.format(VCS_TAG)
  endif
  APPLICATION_ID = '@0@.Devel'.format(PROJECT_RDNN_NAME)
else
  VERSION_SUFFIX = ''
  APPLICATION_ID = PROJECT_RDNN_NAME
endif

# Required dependencies
dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.12.0')
dependency('libadwaita-1', version: '>= 1.5.0')

meson.add_dist_script(
  'build-aux/dist-vendor.sh',
  join_paths(meson.project_build_root(), 'meson-dist', meson.project_name() + '-' + meson.project_version()),
  meson.project_source_root()
)

# Setup pre-commit hook to ensure the coding style is always consistent
if get_option('profile') == 'development'
  message('Setting up git pre-commit hook...')
  run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit', check: false)
  run_command('chmod', '+x', '.git/hooks/pre-commit', check: false)
endif

# Set configuration data
conf = configuration_data()
conf.set('APP_ID', APPLICATION_ID)
conf.set('ROOT_PATH', ROOT_PATH)
conf.set('PKGDATA_DIR', PKGDATA_DIR)
conf.set('LOCALE_DIR', join_paths(get_option('prefix'), get_option('localedir')))
conf.set('VERSION', meson.project_version() + VERSION_SUFFIX)
conf.set('GETTEXT_PACKAGE', meson.project_name())

# Subdirs
subdir('data')
subdir('po')
subdir('src')

# Create devenv if profile is set to 'development'
if get_option('profile') == 'development'
  devenv = environment()

  devenv.prepend('XDG_DATA_DIRS', [
    join_paths(meson.current_build_dir(), get_option('datadir'))
  ])

  meson.add_devenv(devenv)
endif

# Execute post-installation GTK/GNOME scripts
gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
