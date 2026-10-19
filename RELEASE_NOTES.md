## 🦀 zqlcrab v0.1.17

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.17 Release Highlights:
- 🛡️ **Production Safeguard & Read-Only Connection Protection (生产环境安全保护与只读连接模式)**:
  - **Comprehensive Multi-Tier Environment Classification**:
    - Added full environment classification (`DEV`, `TEST`, `STAGE`, `PROD`) across connection configs and profiles.
    - Integrated segmented environment selector and dedicated Read-Only Protection toggle into `ConnectionDialog`.
    - Added prominent theme-coded environment badges across TitleBar, StatusBar, and Sidebar connection lists.
  - **Lexical Safety Analysis & Production Risk Interceptor**:
    - Implemented `QuerySafetyValidator::detect_production_risk` detecting `DROP TABLE/DATABASE/VIEW`, `TRUNCATE TABLE`, `ALTER TABLE`, and unbounded `DELETE`/`UPDATE` operations missing `WHERE` clauses.
    - Introduced high-visibility `ProductionSafeguardModal` with red warning banners, risk diagnosis, target database info, syntax preview, and double-confirmation flow.
  - **Strict Read-Only Connection Guarding**:
    - Pre-flight blocked mutating and destructive DDL/DML statements (`DROP`, `DELETE`, `UPDATE`, `INSERT`, `ALTER`, etc.) on read-only connections with descriptive explanations.
    - Contextually hidden destructive UI operations (`Create Table`, `Restore Database`, `Import Data`, `Mock Data`, `Truncate Table`, `Drop Table/View`) in the sidebar toolbar and table context menus when active connection is read-only.
  - **Enhanced Destructive Action Confirmations**:
    - Upgraded `ConfirmDialog` and `SqlReviewModal` with production-aware warning banners and high-risk badges when targeting production databases.
    - Added full bilingual (English & 简体中文) localization for safeguard dialogs and environment configurations.
