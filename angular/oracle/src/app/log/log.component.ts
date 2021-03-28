import { HttpClient } from '@angular/common/http';
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

  constructor(private http: HttpClient) { }

  ngOnInit(): void {
    // Dummy request to log us out if needed
    this.http.get("/api/dummy").subscribe(_dummy => { });

    this.ws = new WebSocket(`ws://${window.location.host}/api/log`)
    this.ws.onmessage = event => {
      let events = JSON.parse(event.data);
      for (let event of events) {
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
