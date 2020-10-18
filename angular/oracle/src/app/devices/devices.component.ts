import { Component, OnInit } from '@angular/core';

@Component({
  selector: 'app-devices',
  templateUrl: './devices.component.html',
  styleUrls: ['./devices.component.scss']
})
export class DevicesComponent implements OnInit {
  list = [];
  showAdd = false;

  add() {
    this.showAdd = true;
  }

  close() {
    this.showAdd = false;
  }

  constructor() { }

  ngOnInit(): void {
    fetch("/api/devices").then(response => response.json())
      .then(data => {
        this.list = data;
      })
  }

}
