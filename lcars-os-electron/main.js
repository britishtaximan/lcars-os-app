// LCARS OS — Electron Main Process
// Calls shared lcars-metrics Rust binary for identical values to Tauri

var electron = require('electron');
var app = electron.app;
var BrowserWindow = electron.BrowserWindow;
var ipcMain = electron.ipcMain;
var shell = electron.shell;
var path = require('path');
var os = require('os');
var fs = require('fs');
var child_process = require('child_process');
var exec = child_process.exec;

var mainWindow;
var cachedComms = null;
var cachedCommsTime = 0;
var COMMS_CACHE_MS = 30000;

// Path to the shared Rust metrics binary
var metricsPath;
if (app.isPackaged) {
  metricsPath = path.join(path.dirname(app.getAppPath()), '..', 'lcars-metrics');
} else {
  metricsPath = path.join(__dirname, '..', 'lcars-metrics', 'target', 'release', 'lcars-metrics');
}

// === Async shell command helper (non-blocking) ===
function execAsync(command, timeout) {
  return new Promise(function(resolve) {
    exec(command, { encoding: 'utf8', timeout: timeout || 10000 }, function(err, stdout) {
      resolve(err ? '' : stdout);
    });
  });
}

// === Window Creation ===
function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1440,
    height: 900,
    title: 'LCARS OS — Voyager Edition',
    backgroundColor: '#000000',
    titleBarStyle: 'default',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    }
  });
  var indexPath;
  if (app.isPackaged) {
    indexPath = path.join(path.dirname(app.getAppPath()), '..', 'index.html');
  } else {
    indexPath = path.join(__dirname, '..', 'index.html');
  }
  mainWindow.loadFile(indexPath);
}

app.whenReady().then(createWindow);
app.on('window-all-closed', function() { app.quit(); });
app.on('activate', function() { if (BrowserWindow.getAllWindows().length === 0) createWindow(); });

// === System Metrics — calls same Rust sysinfo code as Tauri ===
ipcMain.handle('get_system_metrics', async function() {
  try {
    var output = await execAsync('"' + metricsPath + '" metrics', 8000);
    if (output) {
      return JSON.parse(output);
    }
  } catch(e) {
    console.error('Metrics error:', e);
  }
  return {
    cpu_usage: 0, cpu_brand: 'Unknown', memory_total: 0, memory_used: 0,
    memory_usage_percent: 0, disk_total: 0, disk_used: 0, disk_usage_percent: 0,
    network_rx_bytes: 0, network_tx_bytes: 0, uptime_seconds: os.uptime(),
    battery_percent: -1, battery_charging: false, thermal_pressure: 'NOMINAL'
  };
});

// === Comms Status — calls same Rust code as Tauri ===
ipcMain.handle('get_comms_status', async function() {
  var now = Date.now();
  if (cachedComms && (now - cachedCommsTime) < COMMS_CACHE_MS) {
    return cachedComms;
  }

  try {
    var output = await execAsync('"' + metricsPath + '" comms', 15000);
    if (output) {
      cachedComms = JSON.parse(output);
      cachedCommsTime = Date.now();
      return cachedComms;
    }
  } catch(e) {
    console.error('Comms error:', e);
  }
  return {
    wifi: 'Not Connected',
    bluetooth_enabled: false,
    bluetooth_devices: [],
    volume_percent: -1,
    brightness_percent: -1
  };
});

// === File Browser ===
ipcMain.handle('list_directory', async function(event, args) {
  var dirPath = (args && args.path) ? args.path : os.homedir();
  try {
    var entries = fs.readdirSync(dirPath, { withFileTypes: true });
    var result = [];
    entries.forEach(function(entry) {
      if (entry.name.startsWith('.')) return;
      var fullPath = path.join(dirPath, entry.name);
      var isDir = false;
      var size = 0;
      try {
        isDir = entry.isDirectory();
        if (!isDir) {
          var stat = fs.statSync(fullPath);
          size = stat.size;
        }
      } catch(e) {}
      result.push({ name: entry.name, path: fullPath, is_dir: isDir, size: size });
    });
    return result;
  } catch(e) {
    throw new Error('Cannot read directory: ' + e.message);
  }
});

ipcMain.handle('open_file', async function(event, args) {
  if (args && args.path) shell.openPath(args.path);
});

ipcMain.handle('get_home_dir', async function() {
  return os.homedir();
});

// === App Launcher ===
ipcMain.handle('launch_app', async function(event, args) {
  if (!args || !args.name) return;
  if (process.platform === 'darwin') {
    exec('open -a "' + args.name.replace(/"/g, '\\"') + '"');
  } else if (process.platform === 'win32') {
    exec('start "" "' + args.name + '"');
  } else {
    exec(args.name.toLowerCase());
  }
});

// === Task Persistence ===
var tasksPath = path.join(os.homedir(), '.lcars-os-tasks.json');
var logPath = path.join(os.homedir(), '.lcars-os-captains-log.json');

ipcMain.handle('save_tasks', async function(event, args) {
  try {
    fs.writeFileSync(tasksPath, (args && args.data) ? args.data : '[]', 'utf8');
  } catch(e) {
    throw new Error('Cannot save tasks: ' + e.message);
  }
});

ipcMain.handle('load_tasks', async function() {
  try {
    if (fs.existsSync(tasksPath)) {
      return fs.readFileSync(tasksPath, 'utf8');
    }
    return '[]';
  } catch(e) {
    return '[]';
  }
});

ipcMain.handle('save_log', async function(event, args) {
  try {
    fs.writeFileSync(logPath, (args && args.data) ? args.data : '[]', 'utf8');
  } catch(e) {
    throw new Error('Cannot save log: ' + e.message);
  }
});

ipcMain.handle('load_log', async function() {
  try {
    if (fs.existsSync(logPath)) {
      return fs.readFileSync(logPath, 'utf8');
    }
    return '[]';
  } catch(e) {
    return '[]';
  }
});
