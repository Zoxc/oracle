import '@riotjs/hot-reload'
import { component, register } from 'riot'
import App from './app.riot'

import Settings from './settings.riot'

register('settings', Settings)

component(App)(document.getElementById('root'))
