 Compiling version_check v0.9.5                           
   Compiling zmij v1.0.21                                   
node:internal/fs/watchers:321           ] 28/446: cc, ser…
    const error = new UVException({
                  ^

Error: EBUSY: resource busy or locked, watch 'D:\Users\xgrwx\Downloads\teacher-schedule-widget\src-tauri\target\debug\build\zmij-bbaef9d295c9b9a4\build_script_build-bbaef9d295c9b9a4.exe'
    at FSWatcher.<computed> (node:internal/fs/watchers:321:19)
    at Object.watch (node:fs:2548:36)
    at createFsWatchInstance (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:12958:16)
    at setFsWatchListener (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:13000:14)
    at NodeFsHandler$1._watchWithNodeFs (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:13114:20)
    at NodeFsHandler$1._handleFile (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:13158:24)
    at NodeFsHandler$1._addToNodeFs (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:13332:26)
Emitted 'error' event on FSWatcher instance at:
    at FSWatcher._handleError (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:14133:148)
    at NodeFsHandler$1._addToNodeFs (file:///D:/Users/xgrwx/Downloads/teacher-schedule-widget/node_modules/vite/dist/node/chunks/config.js:13337:18) {
  errno: -4082,
  syscall: 'watch',
  code: 'EBUSY',
  path: 'D:\\Users\\xgrwx\\Downloads\\teacher-schedule-widget\\src-tauri\\target\\debug\\build\\zmij-bbaef9d295c9b9a4\\build_script_build-bbaef9d295c9b9a4.exe',
  filename: 'D:\\Users\\xgrwx\\Downloads\\teacher-schedule-widget\\src-tauri\\target\\debug\\build\\zmij-bbaef9d295c9b9a4\\build_script_build-bbaef9d295c9b9a4.exe'
}

Node.js v24.16.0
   Compiling smallvec v1.15.1                               
       Error The "beforeDevCommand" terminated with a non-zero status code.
PS D:\Users\xgrwx\Downloads\teacher-schedule-widget> 
