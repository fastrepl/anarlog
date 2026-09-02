Pod::Spec.new do |s|
  s.name           = 'AnarlogQuickActions'
  s.version        = '1.0.0'
  s.summary        = 'Exposes Anarlog actions to iOS Shortcuts.'
  s.description    = 'Adds the Start or Stop Listening App Shortcut for the iPhone Action Button.'
  s.author         = 'Anarlog'
  s.homepage       = 'https://anarlog.so'
  s.platform       = :ios, '16.4'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'
  s.frameworks = 'AppIntents'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
  }

  s.source_files = "**/*.{h,m,mm,swift,hpp,cpp}"
end
