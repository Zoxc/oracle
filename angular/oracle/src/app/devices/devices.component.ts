import { Component, OnInit, ViewContainerRef } from '@angular/core';
import { AddDeviceComponent } from './../add-device/add-device.component';
import { NzModalRef, NzModalService } from 'ng-zorro-antd/modal';

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

  constructor(private modal: NzModalService, private viewContainerRef: ViewContainerRef) { }

  add_device(): void {
    const modal = this.modal.create({
      nzTitle: 'Add device',
      nzContent: AddDeviceComponent,
      nzViewContainerRef: this.viewContainerRef,
      nzComponentParams: {},
      nzOnOk: () => new Promise(resolve => setTimeout(resolve, 1000)),
      nzMaskClosable: false,
    });
    const instance = modal.getContentComponent();
    modal.afterClose.subscribe(result => {
      console.log('[afterClose] The result is:', result); if (result) { this.update() }
    });
  }

  delete(id) {
    let device = this.list.find(device => device.id === id)

    this.modal.confirm({
      nzTitle: `Do you want to delete ${device.name || device.ipv4}?`,
      nzOnOk: () => {

        fetch(`/api/device/${device.id}`, {
          method: "DELETE",
        }).then(errors => {
          this.update()
        })
      }
    });
  }

  update(): void {
    fetch("/api/devices").then(response => response.json())
      .then(data => {
        this.list = data;
      })
  }

  ngOnInit(): void {
    this.update()
  }
}

