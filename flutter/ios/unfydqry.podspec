Pod::Spec.new do |s|
  s.name             = 'unfydqry'
  s.version          = '0.1.0'
  s.summary          = 'Flutter plugin for the unfydqry full-text search engine.'
  s.homepage         = 'https://github.com/0x0c/unfydqry'
  s.license          = { :file => '../../LICENSE' }
  s.author           = { 'unfydqry' => 'noreply@example.com' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'Flutter'
  s.platform         = :ios, '16.0'
  s.swift_version    = '5.9'

  # The UnifiedQuery XCFramework bundles the Rust static library and the
  # generated Swift binding.
  #
  # Packaging strategy (A): the framework is built at <repo>/ios/ and *copied*
  # into this pod root (flutter/ios/) by the build step, so the pod stays
  # self-contained and `pod lib lint` works. The copy is gitignored — see
  # "Building native artifacts" in docs/flutter-plugin.md.
  #
  # FUTURE (C): once we cut tagged releases, switch to fetching a prebuilt
  # binary at `pod install` time instead of requiring a local build, e.g.
  #   s.source = { :http => 'https://github.com/0x0c/unfydqry/releases/download/vX.Y.Z/UnifiedQuery.xcframework.zip' }
  # This removes the Rust toolchain requirement for plugin consumers.
  # Tracking: see docs/flutter-plugin.md "Native-artifact packaging".
  s.vendored_frameworks = 'UnifiedQuery.xcframework'
end
