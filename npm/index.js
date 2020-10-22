import '@riotjs/hot-reload'
import { component, register } from 'riot'
import App from './app.riot'
import styles from './style.scss'

import Settings from './settings.riot'
register('settings', Settings)

import Devices from './devices.riot'
register('devices', Devices)

component(App)(document.getElementById('root'))
