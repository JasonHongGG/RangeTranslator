import { spawn } from 'node:child_process'

const args = process.argv.slice(2)
const command = args[0] ?? ''
const forwardedArgs =
  command === 'dev'
    ? [...args, '--config', 'src-tauri/tauri.dev.conf.json']
    : args

const child =
  process.platform === 'win32'
    ? spawn(`npx tauri ${forwardedArgs.map(quoteForShell).join(' ')}`, {
        stdio: 'inherit',
        env: process.env,
        shell: true,
      })
    : spawn('npx', ['tauri', ...forwardedArgs], {
        stdio: 'inherit',
        env: process.env,
      })

function quoteForShell(value) {
  if (!/[\s"]/u.test(value)) {
    return value
  }

  return `"${value.replaceAll('"', '\\"')}"`
}

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal)
    return
  }

  process.exit(code ?? 0)
})
