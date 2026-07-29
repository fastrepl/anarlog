Pod::Spec.new do |s|
  s.name           = 'AnarlogWatchConnectivity'
  s.version        = '1.0.0'
  s.summary        = 'Connects the Anarlog iPhone and watchOS apps.'
  s.description    = 'Transfers account state and watch recordings through Watch Connectivity.'
  s.author         = 'Anarlog'
  s.homepage       = 'https://anarlog.so'
  s.platform       = :ios, '16.4'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'
  s.frameworks = 'WatchConnectivity'

  # Swift/Objective-C compatibility
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
  }

  s.source_files = "**/*.{h,m,mm,swift,hpp,cpp}"
end
