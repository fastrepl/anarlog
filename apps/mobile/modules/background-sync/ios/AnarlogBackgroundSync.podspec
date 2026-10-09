Pod::Spec.new do |s|
  s.name           = 'AnarlogBackgroundSync'
  s.version        = '1.0.0'
  s.summary        = 'Keeps Anarlog sync running after the app leaves the foreground.'
  s.description    = 'Holds background execution time and a continued processing task while sync work is pending.'
  s.author         = 'Anarlog'
  s.homepage       = 'https://anarlog.so'
  s.platform       = :ios, '16.4'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  # Swift/Objective-C compatibility
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
  }

  s.source_files = "**/*.{h,m,mm,swift,hpp,cpp}"
end
