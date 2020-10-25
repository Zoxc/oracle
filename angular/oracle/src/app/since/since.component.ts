import { Component, OnInit, Input } from '@angular/core';
import { DatePipe } from '@angular/common';

@Component({
  selector: 'app-since',
  templateUrl: './since.component.html',
  styleUrls: ['./since.component.scss']
})
export class SinceComponent implements OnInit {
  @Input() since: any
  now: any
  interval: any

  constructor() { }

  ngOnChanges() {
    this.now = Date.now()
  }

  ngOnInit(): void {
    this.interval = setInterval(() => {
      this.now = Date.now()
    }, 1000)
  }

  ngOnDestroy() {
    clearInterval(this.interval)
  }
}
