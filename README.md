<div align="center">

# 🚀 Glitchy Launcher | لانچر ماینکرفت گلیچی
**لانچر مدرن، سریع و هوشمند ماینکرفت**

[![Release](https://img.shields.io/badge/Release-v1.3.1-10b981?style=for-the-badge&logo=github)](https://github.com/GlitchyProjects/GlitchyLauncher/releases)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24c8db?style=for-the-badge&logo=tauri)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61dafb?style=for-the-badge&logo=react)](https://react.dev)
[![Cloudflare](https://img.shields.io/badge/Cloudflare-Workers_%26_D1-f38020?style=for-the-badge&logo=cloudflare)](https://workers.cloudflare.com)
[![Modrinth](https://img.shields.io/badge/Modrinth-Integrated-00af5c?style=for-the-badge&logo=modrinth)](https://modrinth.com)
[![License](https://img.shields.io/badge/License-Repository_LICENSE-blue?style=for-the-badge)](LICENSE)

</div>

---

> [!NOTE]
> ### ℹ️ پایه و اساس پروژه (Foundation & Attribution)
> این لانچر بر پایه پروژه **Falcon Launcher** ساخته شده است.  
> با تشکر از زحمات توسعه‌دهندگان اولیه فالکن لانچر:
> **`Mmd4J`** • **`MrRsd01`** • **`Maploop`** • **`Gnkalk`**

---

## ✨ ویژگی‌ها (Features)

* 🛡️ **حساب کاربری ابری GlitchyAccount** (بر بستر Cloudflare Workers و دیتابیس D1)
* 🔒 **سیستم ضدفیک و مسدودسازی ایمیل‌های موقت** (Anti-Disposable Email)
* 🌐 **معماری اولویت آفلاین (Offline-First)** جهت اجرای بازی بدون نیاز به اینترنت
* 🔄 **همگام‌سازی ابری و هماهنگی لحظه‌ای با سیستم پروفایل ماینکرفت**
* 🤖 **دستیار هوش مصنوعی** جهت عیب‌یابی لاگ‌ها و کرش‌ریپورت‌های بازی
* ⚡ **دکمه رفع خودکار خطا (1-Click Auto-Fix)** بدون نیاز به تنظیمات دستی
* 💬 **چت هوشمند ماینکرفت** جهت راهنمایی مادها، تنظیمات و افزایش FPS
* 🧩 **یکپارچگی مستقیم با مخزن رسمی Modrinth** جهت جستجو و نصب مادها، مادپک‌ها، شیدرها و ریسورس‌پک‌ها
* 🎒 **سیستم کوله‌پشتی (Backpack)** برای مدیریت تفکیک‌شده محتوای هر اینستنس
* 📦 **مدیریت پیشرفته اینستنس‌ها** با ایزوله‌سازی کامل پوشه‌ها، سیوها و فایل‌های بازی
* 🏷️ **امکان انتخاب نام دلخواه و سفارشی** برای هر اینستنس
* 🎮 **پشتیبانی کامل از لودرهای Vanilla، Fabric، Forge، NeoForge و OptiFine**
* 🎨 **استودیو سه‌بعدی اسکین و شنل** با موتور رندر تعاملی skinview3d
* 👕 **پشتیبانی از مدل‌های اسکین Classic (4px) و Slim (3px)** و آپلود مستقیم
* ⚡ **موتور دانلود موازی و پرسرعت** همراه با بررسی خودکار چک‌سام و هش سلامت فایل‌ها
* 🚀 **پشتیبانی از میرورهای پرسرعت** (Official Mojang, BMCLAPI, MCBBS)
* 🔄 **سیستم بررسی نسخه و به‌روزرسانی خودکار** با Silent-Ignore در حالت آفلاین
* 📊 **ثبت آمار زمان بازی، رکوردهای روزانه، سیستم اچیومنت و بج‌ها (Badges)**
* 🌙 **رابط کاربری مدرن نئونی** با پشتیبانی کامل از زبان‌های فارسی (فونت وزیرمتن) و انگلیسی

---

## 🛠️ معماری فنی (Tech Stack)

| بخش | فناوری‌های اصلی |
| :--- | :--- |
| **هسته لانچر (Backend)** | Rust, Tauri v2, Tokio, Reqwest, Serde |
| **رابط کاربری (Frontend)** | React 19, TypeScript, Vite, Tailwind CSS, Base UI, Lucide Icons |
| **سرورلس و ابری (Cloud)** | Cloudflare Workers, Cloudflare D1 Database (SQLite at Edge), Web Crypto |
| **موتور سه‌بعدی (3D)** | Three.js, Skinview3d |
| **ارتباطات و داده** | Cloudflare Edge APIs, GitHub REST API, Modrinth API, Mojang Piston API |

---

## 🚀 راهنمای بیلد و راه‌اندازی (Build & Development)

### پیش‌نیازها:
- [Node.js](https://nodejs.org) (نسخه 20 یا بالاتر)
- [pnpm](https://pnpm.io) (نسخه 9 یا بالاتر)
- [Rust & Cargo](https://rustup.rs) (آخرین نسخه Stable)

### اجرای نسخه توسعه:
```bash
# کلون کردن ریپازیتوری
git clone https://github.com/GlitchyProjects/GlitchyLauncher.git
cd GlitchyLauncher

# نصب پکیج‌های فرانت‌اند
pnpm install

# اجرای محیط توسعه لوکال لانچر
pnpm tauri dev
```

### کامپایل نسخه نهایی (Production Build):
```bash
# بیلد فرانت‌اند
pnpm build

# بیلد باینری Release در Rust
cargo build --release --manifest-path src-tauri/Cargo.toml

# بیلد نصاب اختصاصی Glitchy Launcher WebSetup (ویندوز)
powershell -ExecutionPolicy Bypass -File installer/build.ps1 -CopyToDesktop
```

---

## 📜 لایسنس (License)

این پروژه صرفاً و منحصراً از شرایط و ضوابط مندرج در فایل **[LICENSE](LICENSE)** داخل خود این ریپازیتوری پیروی می‌کند.
