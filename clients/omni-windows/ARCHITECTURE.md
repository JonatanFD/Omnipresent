# Windows GUI Architecture

## Paridad con macOS (contrato)

La app de Windows debe coincidir con la de macOS (`clients/omni-macos`) en
**layout, secciones, opciones y disposición de elementos**. El diseño visual sí
es propio de cada plataforma: aquí se usan tarjetas Fluent y allí `Form
.grouped`. Lo que no puede divergir:

| Aspecto | Regla |
|---------|-------|
| Secciones | General · Connections · System · Update, en ese orden |
| Panel de navegación | Título "Omnipresent", punto de estado en General, badge de pendientes en Connections |
| Título del detalle | El nombre de la sección (equivale a `.navigationTitle`) |
| Orden dentro de cada panel | El mismo que en `MainView.swift` |
| Etiquetas y textos | Idénticos ("Connect to host", "Screen layout", "Port", …) |
| Reglas de visibilidad y habilitado | Las decide `DaemonViewModel`, no el XAML |

`MainView.swift` es la referencia. Cualquier cambio de layout en un lado se
replica en el otro en el mismo cambio.

## Estructura de navegación

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            MainWindow.xaml                               │
│                         (WinUI 3 Window Container)                       │
├─────────────────────────────────────────────────────────────────────────┤
│                            MainView.xaml                                 │
│                      (Navigation & Header Container)                     │
├───────────────────┬─────────────────────────────────────────────────────┤
│  Navigation View  │                   Content Frame                       │
│  ─────────────    │                ─────────────────                     │
│ ┌─────────────┐  │  ┌──────────────────────────────────────────────┐    │
│ │   General   │◄───┤│  GeneralView                                  │    │
│ │             │  │  │  - Status indicator                          │    │
│ │ Connections │◄───┤│  - Start/Stop buttons                        │    │
│ │             │  │  │  - System info (port, fingerprint, version)  │    │
│ │   System    │◄───┤│                                               │    │
│ │             │  │  │  ┌──────────────────────────────────────┐    │    │
│ │   Update    │◄───┤│  │ ConnectionsView                       │    │    │
│ │             │  │  │  │ - Connect to peer                    │    │    │
│ └─────────────┘  │  │  │ - Incoming requests (TOFU)           │    │    │
│                  │  │  │ - Active sessions                    │    │    │
│                  │  │  │ - Known peers (with fingerprints)    │    │    │
│                  │  │  │ - Layout configuration               │    │    │
│                  │  │  │                                       │    │    │
│                  │  │  │ ┌──────────────────────────────────┐ │    │    │
│                  │  │  │ │ SystemView                        │ │    │    │
│                  │  │  │ │ - Clipboard toggle               │ │    │    │
│                  │  │  │ │                                   │ │    │    │
│                  │  │  │ │ ┌──────────────────────────────┐ │ │    │    │
│                  │  │  │ │ │ UpdateView                   │ │ │    │    │
│                  │  │  │ │ │ - Version display            │ │ │    │    │
│                  │  │  │ │ │ - Update button              │ │ │    │    │
│                  │  │  │ │ │                              │ │ │    │    │
│                  │  │  │ │ └──────────────────────────────┘ │ │    │    │
│                  │  │  │ └──────────────────────────────────┘ │    │    │
│                  │  │  └──────────────────────────────────────────┘    │
│                  │  └──────────────────────────────────────────────────┘
└───────────────────┴─────────────────────────────────────────────────────┘
```

## Flujo de datos

```
┌──────────────────────────────────────────────────────────────────────┐
│                         DaemonViewModel                               │
│                    (Punto central de estado)                          │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  ObservableObject Properties:                              │    │
│  │  • Connection (Connected/Connecting/Disconnected/...)      │    │
│  │  • StatusText                                              │    │
│  │  • Fingerprint, Port                                       │    │
│  │  • Capturing, ClipboardSharing                             │    │
│  │  • Sessions, Pending, Peers, Placements (Collections)      │    │
│  │  • DaemonVersion, LastError                                │    │
│  │                                                             │    │
│  │  Commands (async):                                         │    │
│  │  • StartDaemonAsync()                                      │    │
│  │  • StopDaemonAsync()                                       │    │
│  │  • ConnectAsync(host)                                      │    │
│  │  • DisconnectAsync(host)                                   │    │
│  │  • AcceptAsync(selector)                                   │    │
│  │  • RejectAsync(selector)                                   │    │
│  │  • SetLayoutAsync(host, edge)                              │    │
│  │  • SetClipboardAsync(enabled)                              │    │
│  │  • RemovePeerAsync(selector)                               │    │
│  └─────────────────────────────────────────────────────────────┘    │
└──────────────────────┬───────────────────────────────────────────────┘
                       │
         ┌─────────────┼─────────────┬──────────────┬────────────┐
         │             │             │              │            │
      ┌──▼──┐      ┌──▼──┐      ┌──▼──┐       ┌──▼──┐     ┌──▼──┐
      │Gen  │      │Conn │      │Sys  │       │Upd  │     │IPC  │
      │View │      │View │      │View │       │View │     │Clnt │
      └──────┘      └──────┘      └──────┘       └──────┘     └──────┘
         │             │             │              │            │
         └─────────────┼─────────────┴──────────────┴────────────┘
                       │
                ┌──────▼──────┐
                │  IPC Layer  │
                │  (Named Pipe)│
                └──────┬──────┘
                       │
            ┌──────────▼──────────┐
            │  Rust Daemon        │
            │  (omni-runtime)     │
            └─────────────────────┘
```

## Mapeo de vistas a responsabilidades

| Vista | Responsabilidad | Contiene |
|-------|-----------------|----------|
| **GeneralView** | Daemon control y info | Status, Start/Stop, info local, errores |
| **ConnectionsView** | Gestión de conexiones | Connect, pending, sesiones, peers, layout |
| **SystemView** | Configuración global | Clipboard toggle |
| **UpdateView** | Actualizaciones | Version, botón de update |

## Flujo de navegación

```
User clicks nav item
         │
         ▼
MainView.OnNavItemInvoked(NavigationViewItemInvokedEventArgs)
         │
         ▼
Extract tag from clicked item
         │
         ▼
NavigateToSection(tag: string)
         │
         ▼
Reuse the cached pane for the tag, or build it once:
┌────────┴────────┬──────────────┬──────────────┬──────────┐
│                 │              │              │          │
"general"     "connections"  "system"      "update"        │
│                 │              │              │          │
▼                 ▼              ▼              ▼          ▼
GeneralView   ConnectionsView SystemView   UpdateView   (general)
         │
         ▼
NavView.Header = nombre de la sección
ContentFrame.Content = panel
         │
         ▼
View renders with ViewModel bindings
```

## Binding y reactividad

```
DaemonViewModel
      │
      ├─▶ PropertyChanged event fires
      │
      ├─▶ x:Bind Mode=OneWay en Views
      │   (Automático a través del binding)
      │
      ├─▶ Collections (ObservableCollection)
      │   └─▶ ItemsControl se actualiza automáticamente
      │
      ├─▶ Command handlers en code-behind
      │   └─▶ await ViewModel.ConnectAsync(...)
      │
      └─▶ Back to daemon via IPC client
```

## Jerarquía de componentes

```
App
 └─ MainWindow
     └─ MainView
         └─ NavigationView
             ├─ PaneHeader ("Omnipresent")
             ├─ MenuItems
             │   ├─ General      (+ punto de estado)
             │   ├─ Connections  (+ InfoBadge de pendientes)
             │   ├─ System
             │   └─ Update
             ├─ Header (nombre de la sección actual)
             └─ Frame
                 └─ Panel actual (instancia cacheada)
                     ├─ GeneralView
                     ├─ ConnectionsView
                     ├─ SystemView
                     └─ UpdateView
```

No hay barra de título propia: el estado del daemon vive en el punto del panel
de navegación y en la fila "Status" de General, igual que en macOS.

Los paneles se construyen una sola vez y se reutilizan, así navegar y volver no
descarta lo que el usuario escribió.

## Comparación con macOS

```
macOS NavigationSplitView:              Windows NavigationView:
┌─────────┬──────────────┐              ┌──────────┬─────────────┐
│ Sidebar │              │              │   Nav    │             │
│         │  Detail Pane │              │   Menu   │  Content    │
├─────────┤              │              ├──────────┤             │
│General  │ (swaps views)│              │General   │ (swaps      │
│         │              │              │Conn.     │  views)     │
│Conn.    │              │              │System    │             │
│         │              │              │Update    │             │
│System   │              │              │          │             │
│         │              │              │          │             │
│Update   │              │              │          │             │
│         │              │              │          │             │
└─────────┴──────────────┘              └──────────┴─────────────┘
```

## Puntos clave de diseño

### 1. **Single Responsibility Principle**
- Cada vista maneja una sección lógica
- MainView solo orquesta navegación
- DaemonViewModel centraliza lógica de negocio

### 2. **Reactive Data Binding**
- ObservableObject con PropertyChanged
- x:Bind OneWay desde XAML
- Colecciones automáticamente refrescadas

### 3. **Separación de concerns**
- IPC logic en Omni.Ipc
- UI state en Omni.App.Core
- Rendering en Views/

### 4. **Escalabilidad**
- Agregar vista nueva = copiar una existente y registrar en MainView
- Agregar campo al ViewModel = automáticamente disponible en todas las vistas

### 5. **Testability**
- Cada vista puede instantiarse con un mock de ViewModel
- Commands son métodos async públicos
- Sin lógica UI en code-behind (handlers simplemente llaman al ViewModel)
