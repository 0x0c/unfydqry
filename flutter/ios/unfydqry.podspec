Pod::Spec.new do |s|
  s.name             = 'unfydqry'
  s.version          = '0.1.0'
  s.summary          = 'Flutter plugin for the unfydqry full-text search engine.'
  s.homepage         = 'https://github.com/0x0c/unfydqry'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'unfydqry' => 'noreply@example.com' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'Flutter'
  s.platform         = :ios, '16.0'
  s.swift_version    = '5.9'

  # The UnifiedQuery XCFramework bundles the Rust static library and the
  # generated Swift binding. Build it with scripts/build-xcframework.sh first.
  s.vendored_frameworks = '../ios/UnifiedQuery.xcframework'
end
