PS C:\Users\Administrator> cd D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test\
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> git pull
fatal: unable to access 'https://github.com/xgrwx2233/wallpaper-widget-mode-test.git/': Connection timed out after 300022 milliseconds
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> git pull
fatal: unable to access 'https://github.com/xgrwx2233/wallpaper-widget-mode-test.git/': Recv failure: Connection was reset
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> git pull
fatal: unable to access 'https://github.com/xgrwx2233/wallpaper-widget-mode-test.git/': Recv failure: Connection was reset
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> git clone https://github.com/xgrwx2233/wallpaper-widget-mode-test.git
Cloning into 'wallpaper-widget-mode-test'...
fatal: unable to access 'https://github.com/xgrwx2233/wallpaper-widget-mode-test.git/': Failed to connect to github.com port 443 after 21071 ms: Could not connect to server
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> npm install
npm : 无法加载文件 D:\Program Files\nodejs\npm.ps1，因为在此系统上禁止运行脚本。有关详细信息，请参阅 https:/go.microsoft.com/fwlink/?LinkID=135170 中的 about_Execution_Policies。
所在位置 行:1 字符: 1
+ npm install
+ ~~~
    + CategoryInfo          : SecurityError: (:) []，PSSecurityException
    + FullyQualifiedErrorId : UnauthorizedAccess
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> npm
npm : 无法加载文件 D:\Program Files\nodejs\npm.ps1，因为在此系统上禁止运行脚本。有关详细信息，请参阅 https:/go.microsoft.com/fwlink/?LinkID=135170 中的 about_Execution_Policies。
所在位置 行:1 字符: 1
+ npm
+ ~~~
    + CategoryInfo          : SecurityError: (:) []，PSSecurityException
    + FullyQualifiedErrorId : UnauthorizedAccess
PS D:\Users\xgrwx\Downloads\wallpaper-widget-mode-test> node
Welcome to Node.js v24.16.0.
Type ".help" for more information.
> exit
Uncaught ReferenceError: exit is not defined
> quit
Uncaught ReferenceError: quit is not defined
> exit()
Uncaught ReferenceError: exit is not defined
>
