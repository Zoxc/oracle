import { Component, OnInit } from '@angular/core';

@Component({
  selector: 'app-log',
  templateUrl: './log.component.html',
  styleUrls: ['./log.component.scss']
})
export class LogComponent implements OnInit {
  log = []
  loaded = false
  ws: WebSocket

  constructor() { }

  ngOnInit(): void {
    this.ws = new WebSocket(`ws://${window.location.host}/api/log`)
    this.ws.onmessage = event => {
      let events = JSON.parse(event.data);
      for (let event of events) {
        console.log(event)
        this.log = [event].concat(this.log);
      }

    };
    this.ws.onopen = () => {
      this.loaded = true;
    }
  }

  ngOnDestroy() {
    this.ws.close()
  }
}
